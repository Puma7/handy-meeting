//! Explicit local reference using invented text; never reads audio or user transcripts.
pub(super) fn check(args: &[String]) -> crate::AppResult<()> {
    if args.len() != 3 {
        return Err("Provide local endpoint, model and a NEW reference folder".into());
    }
    let root = std::path::Path::new(&args[2]);
    if root.exists() {
        return Err("Reference folder must be new; existing evidence preserved".into());
    }
    std::fs::create_dir_all(root).map_err(|_| "Could not create reference folder")?;
    let sources=vec![meeting_notes::Source{id:"positive".into(),source:"system".into(),start_ms:0,end_ms:30000,text:"Ich heiße Anna. Ich übernehme den Prüfbericht bis Freitag.".into()},meeting_notes::Source{id:"hypothetical-address".into(),source:"system".into(),start_ms:30000,end_ms:60000,text:"Vielleicht könnte Peter irgendwann helfen. Anna, liest du bitte vor?".into()},meeting_notes::Source{id:"no-name".into(),source:"microphone".into(),start_ms:60000,end_ms:90000,text:"Ich bin noch unentschlossen. Möglicherweise klappt es später, aber ich verspreche nichts.".into()}];
    crate::create_json(&root.join("fixture.json"), &sources)?;
    let template = meeting_notes::name_candidates::template().map_err(|e| e.to_string())?;
    let start = std::time::Instant::now();
    let result = meeting_notes::analyze(
        &root.join("analyses"),
        "0123456789abcdef0123456789abcdef",
        sources,
        template,
        meeting_notes::Connection {
            endpoint: args[0].clone(),
            model: args[1].clone(),
            mode: "local".into(),
            cloud_consent: false,
            api_key: String::new(),
        },
        None,
    )
    .map_err(|e| e.to_string())?;
    meeting_notes::name_candidates::validate(&result).map_err(|e| e.to_string())?;
    let names: Vec<_> = result
        .sections
        .iter()
        .flat_map(|s| &s.items)
        .map(|i| i.text.clone())
        .collect();
    let success = names.iter().any(|n| n == "Anna")
        && names
            .iter()
            .all(|n| ["Anna", "Peter"].contains(&n.as_str()))
        && result
            .sections
            .iter()
            .flat_map(|s| &s.items)
            .flat_map(|i| &i.evidence)
            .all(|e| e.source_id != "no-name");
    let proof = serde_json::json!({"success":success,"version":crate::VERSION,"actualLocalModel":true,"syntheticText":true,"model":args[1],"mode":"local","cloudTested":false,"elapsedSeconds":start.elapsed().as_secs_f64(),"names":names,"sourceCategories":["positive self-introduction and commitment","hypothetical mention and addressing another person","noncommittal statement without name"],"speakerAssignmentsMade":0,"limits":"Names are quoted review candidates only. No audio diarization, personal identity, hardware or long-term test claim."});
    crate::create_json(&root.join("reference.json"), &proof)?;
    if !success {
        return Err("The local model did not meet the declared name-candidate reference criteria; evidence retained".into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&proof).map_err(|_| "Reference encoding failed")?
    );
    Ok(())
}
