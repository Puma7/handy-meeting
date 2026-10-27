use crate::{blocking, AppResult, Service};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};
use tauri::State;
fn plans(root: &Path) -> meeting_core::Result<PathBuf> {
    meeting_core::metadata::checked_directory(root)?;
    let folder = root.join("retention-plans");
    match std::fs::create_dir(&folder) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.into()),
    }
    meeting_core::metadata::checked_directory(&folder)
}
#[tauri::command]
pub(crate) async fn get_retention_policy(
    service: State<'_, Arc<Service>>,
) -> AppResult<meeting_core::retention::Policy> {
    let service = Arc::clone(&service);
    blocking(move||meeting_core::retention::read_policy(&service.root).map_err(|_|"Aufräumregeln konnten nicht gelesen werden; vorhandene Einstellungen werden nicht überschrieben. / Retention settings could not be read; existing settings are preserved.".into())).await
}
#[tauri::command]
pub(crate) async fn save_retention_policy(
    service: State<'_, Arc<Service>>,
    expected_revision: u64,
    policy: meeting_core::retention::Policy,
) -> AppResult<meeting_core::retention::Policy> {
    let service = Arc::clone(&service);
    blocking(move||meeting_core::retention::save_policy(&service.root,expected_revision,policy).map_err(|_|"Aufräumregeln nicht gespeichert. Unangetasteten Zeitraum, Altersgrenzen und geänderte Einstellungen prüfen. / Retention settings were not saved; check untouched period, age limits and concurrent changes.".into())).await
}
#[tauri::command]
pub(crate) fn preview_retention(service: State<'_, Arc<Service>>) -> AppResult<String> {
    let owned = Arc::clone(&service);
    service
        .maintenance_jobs
        .start_work("cleanup-preview", move |id, jobs, cancel| {
            let mut plan = meeting_core::retention::preview(
                &owned.root,
                chrono_now(),
                cancel,
                |done, total| jobs.update_work(id, done, total),
            )?;
            plan.id = id.to_owned();
            crate::create_json(&plans(&owned.root)?.join(format!("{id}.json")), &plan)
                .map_err(|e| e.into())
        })
}
fn chrono_now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}
#[tauri::command]
pub(crate) async fn get_retention_plan(
    service: State<'_, Arc<Service>>,
    job_id: String,
) -> AppResult<meeting_core::retention::Plan> {
    let service = Arc::clone(&service);
    blocking(move || {
        crate::validate_id(&job_id)?;
        let folder = plans(&service.root).map_err(|_| "Preview storage unavailable")?;
        let path = folder.join(format!("{job_id}.json"));
        let meta = std::fs::symlink_metadata(&path).map_err(|_| "Preview is not complete yet")?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                return Err("Linked preview rejected".into());
            }
        }
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 32 * 1024 * 1024 {
            return Err("Invalid preview file".into());
        }
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|_| "Preview unavailable")?
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Preview unavailable")?;
        let plan: meeting_core::retention::Plan = serde_json::from_slice(&bytes)
            .map_err(|_| "Preview is invalid; original recordings are preserved")?;
        if plan.id != job_id {
            return Err("Preview identity mismatch; create a new preview".into());
        }
        Ok(plan)
    })
    .await
}
#[tauri::command]
pub(crate) fn get_maintenance_progress(
    service: State<'_, Arc<Service>>,
) -> Option<crate::library_service::Progress> {
    service.maintenance_jobs.snapshot()
}
#[tauri::command]
pub(crate) fn cancel_maintenance(service: State<'_, Arc<Service>>, job_id: String) -> bool {
    service.maintenance_jobs.cancel(&job_id)
}
/// Reserve all operations that could move sources before starting the background job.
struct MaintenanceGuard(Arc<Service>);
impl Drop for MaintenanceGuard {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.0.lock() {
            inner.transitioning = false;
        }
    }
}
fn reserve(service: &Arc<Service>) -> AppResult<MaintenanceGuard> {
    let _gate = service
        .transition
        .lock()
        .map_err(|_| "Recording control unavailable")?;
    service.reap_finished()?;
    let mut inner = service.lock()?;
    if inner.active.is_some()
        || inner.transitioning
        || service.notes_jobs.running()
        || service
            .speaker_jobs
            .snapshot()
            .is_some_and(|p| p.status == "running")
        || service
            .library_jobs
            .snapshot()
            .is_some_and(|p| p.status == "running")
        || service
            .maintenance_jobs
            .snapshot()
            .is_some_and(|p| p.status == "running")
    {
        return Err("Finish recording, recognition, analysis, speaker experiments and search indexing first. / Bitte Aufnahme, Erkennung, Auswertung, Sprecherexperimente und Indexierung zuerst abschließen.".into());
    }
    inner.view = None;
    inner.transitioning = true;
    Ok(MaintenanceGuard(service.clone()))
}
#[tauri::command]
pub(crate) async fn list_archives(
    service: State<'_, Arc<Service>>,
    offset: usize,
    limit: usize,
) -> AppResult<meeting_core::archive::ArchivePage> {
    let service = Arc::clone(&service);
    blocking(move||meeting_core::archive::list(&service.root,offset,limit).map_err(|_|"Archive list could not be read. Existing files are preserved; inspect the local archive folder. / Archivliste konnte nicht gelesen werden. Vorhandene Dateien bleiben erhalten.".into())).await
}
#[tauri::command]
pub(crate) fn restore_archive(
    service: State<'_, Arc<Service>>,
    archive_id: String,
    confirmed: bool,
) -> AppResult<String> {
    if !confirmed {
        return Err("Restoration needs confirmation".into());
    }
    crate::validate_id(&archive_id)?;
    let owned = Arc::clone(&service);
    let guard = reserve(&owned)?;
    service
        .maintenance_jobs
        .start_work("archive-restore", move |id, jobs, cancel| {
            let _guard = guard;
            jobs.update_work(id, 0, 1);
            let folder = meeting_core::archive::restore(&owned.root, &archive_id, cancel)?;
            let data = meeting_core::SessionStore::open(&folder)?.snapshot()?;
            crate::library_service::update(&owned.root, &data)?;
            jobs.update_work(id, 1, 1);
            Ok(())
        })
}
#[tauri::command]
pub(crate) fn compact_archive(
    service: State<'_, Arc<Service>>,
    archive_id: String,
    confirmed: bool,
) -> AppResult<String> {
    if !confirmed {
        return Err("Compaction needs explicit confirmation".into());
    }
    crate::validate_id(&archive_id)?;
    let owned = Arc::clone(&service);
    let guard = reserve(&owned)?;
    service
        .maintenance_jobs
        .start_work("archive-compact", move |id, jobs, cancel| {
            let _guard = guard;
            meeting_core::archive::compact_source_copy(
                &owned.root,
                &archive_id,
                cancel,
                |n, total| jobs.update_work(id, n, total),
            )
        })
}
#[tauri::command]
pub(crate) fn apply_retention(
    service: State<'_, Arc<Service>>,
    plan_id: String,
    candidate_index: usize,
    confirmation: String,
) -> AppResult<String> {
    crate::validate_id(&plan_id)?;
    let owned = Arc::clone(&service);
    let guard = reserve(&owned)?;
    service.maintenance_jobs.start_work("retention-apply",move|id,jobs,cancel|{
        let _guard=guard;
        let folder=plans(&owned.root)?;
        let path=folder.join(format!("{plan_id}.json"));
        let meta=std::fs::symlink_metadata(&path)?;
        #[cfg(windows)]{use std::os::windows::fs::MetadataExt;if meta.file_attributes()&0x400!=0{return Err("Linked preview rejected".into());}}
        if !meta.is_file()||meta.file_type().is_symlink()||meta.len()>32*1024*1024{return Err("Invalid preview".into());}
        let mut bytes=Vec::new();std::fs::File::open(path)?.take(32*1024*1024+1).read_to_end(&mut bytes)?;
        let plan:meeting_core::retention::Plan=serde_json::from_slice(&bytes)?;
        let age=chrono::Utc::now()-chrono::DateTime::parse_from_rfc3339(&plan.created_at)?.with_timezone(&chrono::Utc);
        if plan.id!=plan_id || age<chrono::Duration::zero() || age>chrono::Duration::hours(24){return Err("Preview expired or mismatched; create a new preview".into());}
        let policy=meeting_core::retention::read_policy(&owned.root)?;
        if policy.revision!=plan.policy_revision{return Err("Rules changed; create a new preview".into());}
        let candidate=plan.candidates.get(candidate_index).ok_or("Unknown preview row")?;
        use meeting_core::retention::Action;
        let deleting=candidate.action==Action::DeleteArchivedMeeting;
        let expected=if deleting{format!("DELETE {}",candidate.meeting_id)}else{format!("APPLY {plan_id} {candidate_index}")};
        if confirmation!=expected{return Err("Explicit confirmation did not match the preview action".into());}
        let labels=meeting_core::metadata::read(&owned.root,&candidate.meeting_id)?;
        if labels.pinned||labels.revision!=candidate.metadata_revision{return Err("VIP protection or labels changed; create a new preview".into());}
        let source=owned.root.join("sessions").join(&candidate.meeting_id);
        if !deleting && meeting_core::retention::manifest_fingerprint(&source)?!=candidate.original_manifest_sha256{return Err("Meeting files changed; create a new preview".into());}
        // One immutable preview can be applied once. A partial failure requires a fresh inventory.
        crate::create_json(&folder.join(format!("{plan_id}.applied.json")),&serde_json::json!({"jobId":id,"candidateIndex":candidate_index,"confirmation":confirmation,"at":chrono::Utc::now().to_rfc3339()}))?;
        jobs.update_work(id,0,0);
        match candidate.action {
            Action::ArchiveMeeting=>{
                if meeting_core::archive::read_archived_transcript(&source)?.is_some(){meeting_core::archive::restore_audio(&owned.root,&candidate.meeting_id,cancel)?;}
                let receipt=meeting_core::archive::create(&owned.root,&candidate.meeting_id,labels.revision,"whole-meeting",cancel,|n,total|jobs.update_work(id,n,total))?;
                meeting_core::archive::move_source_into_archive(&owned.root,&receipt,labels.revision,cancel)?;
                meeting_core::library::LibraryIndex::open(&owned.root)?.remove(&candidate.meeting_id)?;
            },
            Action::ArchiveAudio=>{meeting_core::archive::archive_audio(&owned.root,&candidate.meeting_id,labels.revision,cancel,|n,total|jobs.update_work(id,n,total))?;
                let data=meeting_core::archive::read_archived_transcript(&source)?.ok_or("Archived transcript unavailable")?;crate::library_service::update(&owned.root,&data)?;},
            Action::ConvertAudio=>{
                let path=policy.ffmpeg_executable.as_ref().ok_or("Choose a separately installed FFmpeg executable in retention settings first")?;
                let tool=meeting_core::audio_codec::AudioTool::inspect(path,cancel)?;
                let store=meeting_core::SessionStore::open(&source)?;let data=store.snapshot()?;
                if data.status!="complete"||data.read_only||data.capture_incomplete{return Err("Restore archived audio and finish the meeting before converting".into());}
                let base=source.join("converted-audio");match std::fs::create_dir(&base){Ok(())=>(),Err(e)if e.kind()==std::io::ErrorKind::AlreadyExists=>(),Err(e)=>return Err(e.into())}
                meeting_core::metadata::checked_directory(&base)?;
                let output=base.join(id);std::fs::create_dir(&output)?;
                let mut proofs=Vec::new();jobs.update_work(id,0,data.parts.len()as u64);
                for part in &data.parts{proofs.push(tool.convert(&source,part,&output,policy.codec,cancel)?);jobs.update_work(id,proofs.len()as u64,data.parts.len()as u64);}
                crate::create_json(&output.join("conversion.json"),&serde_json::json!({"meetingId":candidate.meeting_id,"jobId":id,"originalsRetained":true,"codec":policy.codec,"parts":proofs}))?;
                let catalog=owned.root.join("audio-copies");match std::fs::create_dir(&catalog){Ok(())=>(),Err(e)if e.kind()==std::io::ErrorKind::AlreadyExists=>(),Err(e)=>return Err(e.into())}
                meeting_core::metadata::checked_directory(&catalog)?;
                crate::create_json(&catalog.join(format!("{id}.json")),&AudioCopy{run_id:id.into(),meeting_id:candidate.meeting_id.clone(),title:candidate.title.clone(),codec:policy.codec,created_at:chrono::Utc::now().to_rfc3339(),sections:proofs.len()})?;
            },
            Action::DeleteArchivedMeeting=>{
                meeting_core::archive::delete_archived_meeting(&owned.root,&candidate.meeting_id,labels.revision,&candidate.original_manifest_sha256,&confirmation,cancel,|n,total|jobs.update_work(id,n,total))?;
                meeting_core::library::LibraryIndex::open(&owned.root)?.remove(&candidate.meeting_id)?;
            }
        }
        Ok(())
    })
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AudioCopy {
    run_id: String,
    meeting_id: String,
    title: String,
    codec: meeting_core::audio_codec::Codec,
    created_at: String,
    sections: usize,
}
fn read_record<T: serde::de::DeserializeOwned>(path: &Path) -> meeting_core::Result<T> {
    let meta = std::fs::symlink_metadata(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err("Linked record rejected".into());
        }
    }
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 32 * 1024 * 1024 {
        return Err("Invalid retained record".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("Oversized retained record".into());
    }
    Ok(serde_json::from_slice(&bytes)?)
}
#[tauri::command]
pub(crate) async fn list_audio_copies(
    service: State<'_, Arc<Service>>,
    offset: usize,
) -> AppResult<Vec<AudioCopy>> {
    let owned = Arc::clone(&service);
    blocking(move || {
        if offset > 1000000 {
            return Err("Invalid audio-copy page".into());
        }
        let folder = owned.root.join("audio-copies");
        if !folder.exists() {
            return Ok(Vec::new());
        }
        meeting_core::metadata::checked_directory(&folder)
            .map_err(|_| "Audio-copy catalog unavailable")?;
        let mut names = Vec::new();
        for entry in std::fs::read_dir(&folder).map_err(|_| "Audio-copy catalog unavailable")? {
            let path = entry.map_err(|_| "Audio-copy catalog unavailable")?.path();
            if path.extension().is_some_and(|v| v == "json") {
                names.push(path);
            }
            if names.len() > 1000000 {
                return Err("Audio-copy catalog exceeds bound".into());
            }
        }
        names.sort();
        let mut output = Vec::new();
        for path in names.iter().skip(offset).take(25) {
            let value: AudioCopy = read_record(path)
                .map_err(|_| "Audio-copy catalog is unreadable; retained files preserved")?;
            crate::validate_id(&value.run_id)?;
            crate::validate_id(&value.meeting_id)?;
            if path.file_stem().and_then(|v| v.to_str()) != Some(value.run_id.as_str()) {
                return Err("Audio-copy identity mismatch".into());
            }
            output.push(value);
        }
        Ok(output)
    })
    .await
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Conversion {
    meeting_id: String,
    job_id: String,
    originals_retained: bool,
    codec: meeting_core::audio_codec::Codec,
    parts: Vec<meeting_core::audio_codec::ConversionProof>,
}
#[tauri::command]
pub(crate) fn transcribe_audio_copy(
    service: State<'_, Arc<Service>>,
    run_id: String,
    model_id: String,
    confirmed: bool,
) -> AppResult<String> {
    if !confirmed {
        return Err("New transcription needs confirmation".into());
    }
    crate::validate_id(&run_id)?;
    let owned = Arc::clone(&service);
    let guard = reserve(&owned)?;
    service.maintenance_jobs.start_work("prepare-transcription",move|id,jobs,cancel|{
        let catalog=meeting_core::metadata::checked_directory(&owned.root.join("audio-copies"))?;
        let copy:AudioCopy=read_record(&catalog.join(format!("{run_id}.json")))?;
        if copy.run_id!=run_id{return Err("Audio-copy identity mismatch".into());}crate::validate_id(&copy.meeting_id)?;
        let policy=meeting_core::retention::read_policy(&owned.root)?;
        let tool=meeting_core::audio_codec::AudioTool::inspect(policy.ffmpeg_executable.as_ref().ok_or("Choose the separately installed FFmpeg tool first")?,cancel)?;
        let source=meeting_core::metadata::checked_directory(&owned.root.join("sessions").join(&copy.meeting_id)).map_err(|_|"Restore the archived whole meeting before using its compact audio copies")?;
        let data=match meeting_core::archive::read_archived_transcript(&source)?{Some(v)=>v,None=>meeting_core::inspect_manifest(&source)?};
        let copies=meeting_core::metadata::checked_directory(&source.join("converted-audio"))?;
        let folder=meeting_core::metadata::checked_directory(&copies.join(&run_id))?;
        let conversion:Conversion=read_record(&folder.join("conversion.json"))?;
        if conversion.meeting_id!=copy.meeting_id||conversion.job_id!=run_id||!conversion.originals_retained||conversion.codec!=copy.codec||conversion.parts.len()!=data.parts.len()||conversion.parts.len()!=copy.sections||data.parts.is_empty(){return Err("Converted audio inventory does not match the original session".into());}
        let (bridge,catalog,_)=owned.refresh(None)?;let model=catalog.model(&model_id)?;
        let resolved=bridge.resolve_streaming_model(model)?;
        let store=meeting_core::SessionStore::create(&owned.root.join("sessions"),&format!("{} · new transcription",copy.title),&model.id,&data.output_name,data.microphone_name.as_deref())?;
        let new_id=store.snapshot()?.id;
        handy_bridge::write_model_snapshot(store.folder(),model,&resolved,&serde_json::json!({"applicationVersion":crate::VERSION,"operation":"new transcription from verified compact audio","sourceMeetingId":copy.meeting_id,"copyRunId":run_id,"codec":copy.codec,"lossy":copy.codec==meeting_core::audio_codec::Codec::Opus,"nativeRuntime":handy_bridge::native::runtime_identity()}))?;
        crate::create_json(&store.folder().join("retranscription-source.json"),&serde_json::json!({"sourceMeetingId":copy.meeting_id,"copyRunId":run_id,"newMeetingId":new_id,"preparedBy":id,"originalStartedAt":data.started_at,"originalsRetained":true,"wordTimes":false,"parts":conversion.parts}))?;
        jobs.update_work(id,0,data.parts.len()as u64);let mut done=0;
        for track in ["system","microphone"]{
            let mut parts:Vec<_>=data.parts.iter().filter(|p|p.source==track).collect();parts.sort_by_key(|p|p.start_sample);
            if parts.is_empty(){continue;}
            let mut writer=meeting_core::PcmWriter::new(store.clone(),track)?;
            for part in parts{
                if cancel.load(std::sync::atomic::Ordering::Acquire){return Err("Preparation cancelled; originals and the partial new session are retained".into());}
                if writer.written_samples()>part.start_sample{return Err("Overlapping source sections; new copy retained for inspection".into());}
                let mut proofs=conversion.parts.iter().filter(|p|p.source_id==part.file);
                let proof=proofs.next().ok_or("Missing converted section")?;if proofs.next().is_some(){return Err("Duplicate converted section".into());}
                let pcm=tool.decode_copy(&folder,proof,part,cancel)?;writer.pad_to(part.start_sample)?;
                for packet in pcm.chunks(16000){if cancel.load(std::sync::atomic::Ordering::Acquire){return Err("Preparation cancelled; originals and partial new copy retained".into());}writer.append(packet)?;}
                done+=1;jobs.update_work(id,done,data.parts.len()as u64);
            }
            writer.finish()?;
        }
        store.set_status("recorded")?;crate::library_service::update(&owned.root,&store.snapshot()?)?;drop(store);
        drop(guard);
        owned.resume(new_id)?;
        Ok(())
    })
}
