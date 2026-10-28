#!/usr/bin/env python3
"""Explicit, Windows-only, real-time native-worker memory experiment.

No device is opened. Only directly created workers are terminated. Two immutable
synthetic 30 s WAVs are replayed through the production worker protocols. This is
not a capture, checkpoint recovery, power-loss or full-application soak test.
Python standard library only; no global tool or security settings are changed.
"""
import argparse
import ctypes as C
from ctypes import wintypes as W
import hashlib
import json
import math
import os
from pathlib import Path
import queue
import re
import statistics
import subprocess
import threading
import time
import wave

MIB = 1024 * 1024


def sha(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as file:
        for block in iter(lambda: file.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def write_json(path, value):
    temporary = path.with_suffix(path.suffix + '.pending')
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding='utf-8')
    for attempt in range(50):
        try:
            os.replace(temporary, path)
            return
        except PermissionError:
            if attempt == 49:
                raise
            time.sleep(.02)


def trend(rows, metric, warmup=120):
    points = [(row['elapsedSeconds'], row.get(metric)) for row in rows
              if row['elapsedSeconds'] >= warmup and row.get(metric) is not None]
    if len(points) < 10:
        return {'available': False, 'samples': len(points)}
    first_time, last_time = points[0][0], points[-1][0]
    first = [v / MIB for t, v in points if t <= first_time + 60]
    last = [v / MIB for t, v in points if t >= last_time - 60]
    mean_t = statistics.mean(t for t, _ in points)
    mean_v = statistics.mean(v / MIB for _, v in points)
    denominator = sum((t - mean_t) ** 2 for t, _ in points)
    slope = sum((t - mean_t) * (v / MIB - mean_v) for t, v in points) / denominator * 3600 if denominator else 0
    return {'available': True, 'samples': len(points), 'observedSeconds': last_time - first_time,
            'firstMedianMiB': statistics.median(first), 'lastMedianMiB': statistics.median(last),
            'growthMiB': statistics.median(last) - statistics.median(first), 'slopeMiBPerHour': slope,
            'peakMiB': max(v for _, v in points) / MIB}


def summarize_trends(phase_rows):
    """PIDs identify a process only inside its measured phase, including the controller."""
    result = {}
    for phase, rows in phase_rows.items():
        phase_result = result[phase] = {}
        for pid in sorted({row['pid'] for row in rows}):
            selected = [row for row in rows if row['pid'] == pid]
            phase_result[str(pid)] = {'phase': phase, 'role': selected[0]['role'], 'backend': selected[0]['backend'],
                **{metric: trend(selected, metric) for metric in ('privateBytes', 'workingSetBytes', 'gpuDedicatedBytes', 'gpuSharedBytes')}}
    return result


def owned_gpu_values(items, pids):
    result = {pid: None for pid in pids}
    for name, value, valid in items:
        match = re.match(r'^pid_(\d+)_', name, re.IGNORECASE)
        if match and int(match[1]) in result and valid and value >= 0:
            pid = int(match[1])
            result[pid] = (result[pid] or 0) + value
    return result


class Memory:
    def __init__(self):
        self.kernel = C.WinDLL('kernel32', use_last_error=True)
        self.psapi = C.WinDLL('psapi', use_last_error=True)
        self.kernel.OpenProcess.argtypes = [W.DWORD, W.BOOL, W.DWORD]
        self.kernel.OpenProcess.restype = W.HANDLE
        self.kernel.CloseHandle.argtypes = [W.HANDLE]
        size = C.c_size_t

        class Counters(C.Structure):
            _fields_ = [('cb', W.DWORD), ('PageFaultCount', W.DWORD)] + [
                (name, size) for name in ('PeakWorkingSetSize', 'WorkingSetSize',
                'QuotaPeakPagedPoolUsage', 'QuotaPagedPoolUsage', 'QuotaPeakNonPagedPoolUsage',
                'QuotaNonPagedPoolUsage', 'PagefileUsage', 'PeakPagefileUsage', 'PrivateUsage')]

        self.Counters = Counters
        self.psapi.GetProcessMemoryInfo.argtypes = [W.HANDLE, C.POINTER(Counters), W.DWORD]
        self.psapi.GetProcessMemoryInfo.restype = W.BOOL

    def read(self, pid):
        handle = self.kernel.OpenProcess(0x0410, False, pid)
        if not handle:
            return {'privateBytes': None, 'workingSetBytes': None, 'memoryError': C.get_last_error()}
        try:
            counters = self.Counters()
            counters.cb = C.sizeof(counters)
            if not self.psapi.GetProcessMemoryInfo(handle, C.byref(counters), counters.cb):
                return {'privateBytes': None, 'workingSetBytes': None, 'memoryError': C.get_last_error()}
            return {'privateBytes': counters.PrivateUsage, 'workingSetBytes': counters.WorkingSetSize}
        finally:
            self.kernel.CloseHandle(handle)


class GpuMemory:
    """PDH GPU Process Memory counters; absent/invalid counters stay null, not zero.

    https://learn.microsoft.com/windows/win32/api/pdh/nf-pdh-pdhaddenglishcounterw
    https://learn.microsoft.com/windows/win32/api/pdh/nf-pdh-pdhgetformattedcounterarrayw
    Other process counters are discarded and never written to the experiment.
    """
    def __init__(self):
        self.pdh = C.WinDLL('pdh')
        self.query = W.HANDLE()
        self.pdh.PdhOpenQueryW.argtypes = [W.LPCWSTR, C.c_size_t, C.POINTER(W.HANDLE)]
        self.pdh.PdhAddEnglishCounterW.argtypes = [W.HANDLE, W.LPCWSTR, C.c_size_t, C.POINTER(W.HANDLE)]
        self.pdh.PdhCollectQueryData.argtypes = [W.HANDLE]
        self.pdh.PdhCloseQuery.argtypes = [W.HANDLE]
        self.pdh.PdhGetFormattedCounterArrayW.argtypes = [W.HANDLE, W.DWORD, C.POINTER(W.DWORD), C.POINTER(W.DWORD), C.c_void_p]
        for name in ('PdhOpenQueryW', 'PdhAddEnglishCounterW', 'PdhCollectQueryData', 'PdhCloseQuery', 'PdhGetFormattedCounterArrayW'):
            getattr(self.pdh, name).restype = W.DWORD

        class Value(C.Union):
            _fields_ = [('large', C.c_longlong), ('double', C.c_double)]

        class Format(C.Structure):
            _fields_ = [('status', W.DWORD), ('value', Value)]

        class Item(C.Structure):
            _fields_ = [('name', W.LPWSTR), ('format', Format)]

        self.Item = Item
        self.counters = {}
        self.error = self.pdh.PdhOpenQueryW(None, 0, C.byref(self.query))
        if not self.error:
            for key, counter_name in [('gpuDedicatedBytes', 'Dedicated Usage'), ('gpuSharedBytes', 'Shared Usage')]:
                handle = W.HANDLE()
                status = self.pdh.PdhAddEnglishCounterW(self.query, '\\GPU Process Memory(*)\\' + counter_name, 0, C.byref(handle))
                self.counters[key] = (handle, status)
            self.pdh.PdhCollectQueryData(self.query)

    def read(self, pids):
        result = {pid: {'gpuDedicatedBytes': None, 'gpuSharedBytes': None} for pid in pids}
        if self.error:
            return result
        self.pdh.PdhCollectQueryData(self.query)
        for key, (handle, status) in self.counters.items():
            if status:
                continue
            size, count = W.DWORD(), W.DWORD()
            status = self.pdh.PdhGetFormattedCounterArrayW(handle, 0x400, C.byref(size), C.byref(count), None)
            if status != 0x800007D2 or not 0 < size.value <= 2 * 1024 * 1024:
                continue
            buffer = C.create_string_buffer(size.value)
            if self.pdh.PdhGetFormattedCounterArrayW(handle, 0x400, C.byref(size), C.byref(count), buffer):
                continue
            if count.value * C.sizeof(self.Item) > len(buffer):
                continue
            items = C.cast(buffer, C.POINTER(self.Item))
            filtered = owned_gpu_values(((items[i].name or '', items[i].format.value.large,
                                        items[i].format.status in (0, 1)) for i in range(count.value)), pids)
            for pid, value in filtered.items():
                result[pid][key] = value
        return result

    def close(self):
        if self.query:
            self.pdh.PdhCloseQuery(self.query)
            self.query = W.HANDLE()


class Worker:
    def __init__(self, command, label, root, env):
        self.label = label
        self.stderr = open(root / (label + '-stderr.log'), 'xb')
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=subprocess.PIPE, env=env, creationflags=subprocess.CREATE_NO_WINDOW)
        self.replies = queue.Queue(maxsize=8)
        self.reader = threading.Thread(target=self._read, daemon=True)
        self.reader.start()
        self.stderr_reader = threading.Thread(target=self._drain_stderr, daemon=True)
        self.stderr_reader.start()

    def _drain_stderr(self):
        kept = 0
        while True:
            block = self.process.stderr.read(4096)
            if not block:
                return
            remainder = max(0, 65536 - kept)
            self.stderr.write(block[:remainder])
            kept += min(len(block), remainder)

    def _read(self):
        try:
            while True:
                line = self.process.stdout.readline(2 * 1024 * 1024 + 1)
                if not line:
                    self.replies.put(RuntimeError('Owned worker closed stdout'))
                    return
                if len(line) > 2 * 1024 * 1024 or not line.endswith(b'\n'):
                    raise RuntimeError('Worker reply exceeded its bounded JSONL contract')
                self.replies.put(json.loads(line.decode('utf-8-sig')))
        except Exception as error:
            self.replies.put(error)

    def receive(self, timeout=180):
        reply = self.replies.get(timeout=timeout)
        if isinstance(reply, Exception):
            raise reply
        if not reply.get('ok', reply.get('Ok', False)):
            raise RuntimeError('Owned worker rejected test request: ' + str(reply.get('error', reply.get('Error'))))
        return reply

    def call(self, command):
        self.process.stdin.write(json.dumps(command, separators=(',', ':')).encode() + b'\n')
        self.process.stdin.flush()
        return self.receive(timeout=90)

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.kill()  # Only this directly created test process.
            self.process.wait(timeout=5)
        self.process.stdout.close()
        self.stderr_reader.join(timeout=5)
        self.process.stderr.close()
        self.stderr.close()


def run_phase(args, root, session, phase, seconds, report):
    workers, owners = [], {}
    sampler_stop = threading.Event()
    sampler = None
    sampler_errors = []
    started = time.monotonic()
    env = dict(os.environ)
    env.setdefault('VK_LOADER_LAYERS_DISABLE', '~implicit~')
    try:
        if phase == 'legacy':
            worker = Worker([str(args.legacy), '--stream-worker'], 'legacy', root, env)
            workers.append(worker)
            ready = worker.call({'Operation': 'load', 'ModelPath': str(args.model), 'CpuOnly': False})
            backend = ready.get('Backend', ready.get('backend', ''))
            owners = {worker.process.pid: {'role': 'legacy-two-stream-worker', 'backend': backend}}
        else:
            for source in ('system', 'microphone'):
                worker = Worker([str(args.rust), '--asr-worker', str(args.model), str(args.rust.parent), str(session), source], 'rust-' + source, root, env)
                workers.append(worker)
                ready = worker.receive()
                owners[worker.process.pid] = {'role': 'rust-' + source, 'backend': ready.get('backend', '')}
        if any('CPU' in entry['backend'].upper() or not entry['backend'] for entry in owners.values()):
            raise RuntimeError('GPU experiment did not select a GPU backend; no silent CPU replacement')
        phase_start = time.monotonic()
        write_json(root / 'status.json', {'phase': phase, 'state': 'running', 'startedAtUnix': time.time(), 'durationSeconds': seconds, 'workers': owners})

        def sample():
            memory, gpu = Memory(), GpuMemory()
            try:
                with open(root / (phase + '-memory.jsonl'), 'x', encoding='utf-8') as output:
                    while not sampler_stop.is_set():
                        now = time.monotonic()
                        gpu_values = gpu.read(owners)
                        for pid, entry in list(owners.items()) + [(os.getpid(), {'role': 'probe-controller', 'backend': None})]:
                            row = {'utcUnix': time.time(), 'phase': phase, 'elapsedSeconds': now - phase_start, 'pid': pid,
                                   **entry, **memory.read(pid), **gpu_values.get(pid, {'gpuDedicatedBytes': None, 'gpuSharedBytes': None})}
                            output.write(json.dumps(row) + '\n')
                        output.flush()
                        sampler_stop.wait(max(0, 1 - (time.monotonic() - now)))
            except Exception as error:
                sampler_errors.append(type(error).__name__ + ': ' + str(error))
            finally:
                gpu.close()

        sampler = threading.Thread(target=sample, daemon=True)
        sampler.start()
        per_source = {'system': 0, 'microphone': 0}
        with open(root / (phase + '-windows.jsonl'), 'x', encoding='utf-8') as output:
            for window in range(seconds // 30):
                for source_index, source in enumerate(('system', 'microphone')):
                    worker = workers[0] if phase == 'legacy' else workers[source_index]
                    command = {'Operation': 'begin', 'Source': source} if phase == 'legacy' else {'operation': 'begin', 'file': source + '-000000000000.wav'}
                    worker.call(command)
                for packet in range(60):
                    if sampler_errors:
                        raise RuntimeError('Memory sampler failed: ' + sampler_errors[0])
                    # Both tracks advance in real time. A slow backend is measured, not disguised by dropping packets.
                    target = phase_start + window * 30 + (packet + 1) * .5
                    time.sleep(max(0, target - time.monotonic()))
                    for source_index, source in enumerate(('system', 'microphone')):
                        worker = workers[0] if phase == 'legacy' else workers[source_index]
                        if phase == 'legacy':
                            command = {'Operation': 'feed', 'Source': source, 'AudioPath': str(session / 'audio' / (source + '-000000000000.wav')), 'OffsetSamples': packet * 8000, 'SampleCount': 8000}
                        else:
                            command = {'operation': 'feed', 'offsetSamples': packet * 8000, 'sampleCount': 8000, 'confirmedSamples': 480000}
                        worker.call(command)
                        per_source[source] += 8000
                for source_index, source in enumerate(('system', 'microphone')):
                    worker = workers[0] if phase == 'legacy' else workers[source_index]
                    reply = worker.call({'Operation': 'finalize', 'Source': source} if phase == 'legacy' else {'operation': 'finalize'})
                    text = reply.get('full', reply.get('Full', ''))
                    if not text.strip():
                        raise RuntimeError('Synthetic speech produced no final text')
                    output.write(json.dumps({'window': window, 'source': source, 'text': text, 'elapsedSeconds': time.monotonic() - phase_start}, ensure_ascii=False) + '\n')
                output.flush()
        report[phase] = {'complete': True, 'durationSeconds': time.monotonic() - phase_start, 'requestedSeconds': seconds, 'samplesPerSource': per_source, 'workers': owners, 'loadSeconds': phase_start - started}
    finally:
        sampler_stop.set()
        if sampler:
            sampler.join(timeout=5)
        for worker in workers:
            worker.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('legacy', 'rust', 'model', 'fixture_session', 'snapshot', 'output'):
        parser.add_argument('--' + name.replace('_', '-'), type=Path, required=True)
    parser.add_argument('--seconds-per-phase', type=int, default=1800)
    args = parser.parse_args()
    if os.name != 'nt':
        parser.error('Native process/GPU measurements require Windows')
    if args.seconds_per_phase < 30 or args.seconds_per_phase > 7200 or args.seconds_per_phase % 30:
        parser.error('Duration must be a multiple of30 in30..7200 seconds')
    args.output.mkdir(parents=True, exist_ok=False)
    root = args.output.resolve()
    session = root / 'synthetic-input'
    (session / 'audio').mkdir(parents=True)
    snapshot = json.loads(args.snapshot.read_text(encoding='utf-8-sig'))
    (session / 'model-snapshot.json').write_text(json.dumps(snapshot), encoding='utf-8')
    inputs = {}
    for source in ('system', 'microphone'):
        path = args.fixture_session / 'audio' / (source + '-000000000000.wav')
        with wave.open(str(path), 'rb') as input:
            if input.getparams()[:4] != (1, 2, 16000, 480000):
                raise ValueError('Require exactly30s mono16kHz16bit synthetic fixtures')
            pcm = input.readframes(480000)
        selected = session / 'audio' / path.name
        with wave.open(str(selected), 'wb') as output:
            output.setparams((1, 2, 16000, 0, 'NONE', 'not compressed'))
            output.writeframes(pcm)
        inputs[source] = {'wavSHA256': sha(selected), 'pcmSHA256': hashlib.sha256(pcm).hexdigest()}
    if inputs['system']['pcmSHA256'] == inputs['microphone']['pcmSHA256']:
        raise ValueError('Two distinct synthetic fixtures are required')
    plan = {'scope': 'Sequential, isolated native-worker experiment; no capture/UI/recovery test',
            'preparedAtUnix': time.time(), 'secondsPerPhase': args.seconds_per_phase, 'samplingSeconds': 1,
            'warmupExcludedSeconds': 120, 'medianWindowSeconds': 60,
            'workerPrivateGrowthLimitMiB': 128, 'workerPrivateSlopeLimitMiBPerHour': 128,
            'workerWorkingSetGrowthLimitMiB': 128, 'gpuThreshold': None,
            'gpuInterpretation': 'Dedicated and shared PDH counters reported separately; missing is null, no zero substitution',
            'legacySHA256': sha(args.legacy), 'legacyAssemblySHA256': sha(args.legacy.with_suffix('.dll')),
            'rustSHA256': sha(args.rust), 'nativeLegacySHA256': sha(args.legacy.parent / 'native' / 'transcribe.dll'),
            'nativeRustSHA256': sha(args.rust.parent / 'transcribe.dll'), 'modelSHA256': sha(args.model),
            'inputs': inputs, 'controllerSHA256': sha(Path(__file__)),
            'runtimePathsAreLocalOnly': True, 'physicalAudioDevicesOpened': False}
    rust_manifest = args.rust.parent / 'BUILD_MANIFEST.json'
    if rust_manifest.is_file():
        provenance = json.loads(rust_manifest.read_text(encoding='utf-8-sig'))
        plan['rustVersion'] = provenance['version']
        plan['rustSourceSHA256'] = provenance['sourceFingerprintSHA256']
    write_json(root / 'plan.json', plan)
    report = {'complete': False, 'scope': plan['scope']}
    try:
        for phase in ('legacy', 'rust'):
            run_phase(args, root, session, phase, args.seconds_per_phase, report)
            write_json(root / 'result.json', report)
        report['complete'] = True
        report['trends'] = summarize_trends({phase: [json.loads(line) for line in
            (root / (phase + '-memory.jsonl')).read_text().splitlines()] for phase in ('legacy', 'rust')})
        write_json(root / 'result.json', report)
        write_json(root / 'status.json', {'state': 'complete', 'atUnix': time.time()})
    except Exception as error:
        report['error'] = type(error).__name__ + ': ' + str(error)
        write_json(root / 'result.json', report)
        write_json(root / 'status.json', {'state': 'failed', 'atUnix': time.time(), 'error': report['error']})
        raise


if __name__ == '__main__':
    main()
