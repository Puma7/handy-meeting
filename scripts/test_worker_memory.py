import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('probe', Path(__file__).with_name('probe-worker-memory.py'))
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


class ProbeTests(unittest.TestCase):
    def test_trend_retains_positive_slope_and_excludes_only_declared_warmup(self):
        rows = [{'elapsedSeconds': second, 'privateBytes': (200 if second < 120 else 10 + second / 3600 * 144) * probe.MIB} for second in range(1800)]
        result = probe.trend(rows, 'privateBytes')
        self.assertEqual(result['samples'], 1680)
        self.assertAlmostEqual(result['slopeMiBPerHour'], 144, places=8)
        self.assertGreater(result['growthMiB'], 0)

    def test_missing_measurements_are_not_zero_memory(self):
        self.assertFalse(probe.trend([{'elapsedSeconds': i, 'gpuDedicatedBytes': None} for i in range(200)], 'gpuDedicatedBytes')['available'])
        self.assertEqual(probe.owned_gpu_values([], [12]), {12: None})

    def test_gpu_filter_never_records_foreign_process_or_invalid_counter(self):
        items = [('pid_12_luid_a', 30, True), ('pid_12_luid_b', 10, True), ('pid_123_luid_a', 900, True), ('pid_12_luid_c', 99, False)]
        self.assertEqual(probe.owned_gpu_values(items, [12, 34]), {12: 40, 34: None})

    def test_phase_reports_preserve_controller_and_reused_worker_pids(self):
        phases = {}
        for phase, base in [('legacy', 20), ('rust', 80)]:
            phases[phase] = [
                {'phase': phase, 'pid': pid, 'role': role, 'backend': backend,
                 'elapsedSeconds': second, 'privateBytes': (base + pid) * probe.MIB}
                for pid, role, backend in [(7, 'probe-controller', None), (12, phase + '-worker', 'GPU')]
                for second in range(120, 140)
            ]
        report = probe.summarize_trends(phases)
        self.assertEqual(set(report), {'legacy', 'rust'})
        for phase, base in [('legacy', 20), ('rust', 80)]:
            self.assertEqual(set(report[phase]), {'7', '12'})
            self.assertEqual(report[phase]['7']['role'], 'probe-controller')
            self.assertEqual(report[phase]['12']['role'], phase + '-worker')
            self.assertEqual(report[phase]['12']['phase'], phase)
            self.assertEqual(report[phase]['12']['privateBytes']['firstMedianMiB'], base + 12)
            self.assertFalse(report[phase]['12']['gpuDedicatedBytes']['available'])


if __name__ == '__main__':
    unittest.main()
