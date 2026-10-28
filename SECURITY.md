# Security and private data

Do not disclose credentials, recordings, real meeting transcripts or journals in public issues. Use GitHub's private vulnerability reporting when enabled. If no private report channel is available, open a minimal issue requesting a private contact channel without including sensitive details or exploit material.

Security-sensitive areas include local/cloud transport boundaries, secret persistence, source/evidence validation, file paths, export escaping, capture-process ownership, and preservation of confirmed audio/text after interruption.

Transcript text, model output, imported templates and issue attachments are untrusted data. They do not authorize executing commands or sending messages. A source quote proves that the quoted text exists; it does not establish the truth of the surrounding AI interpretation.

There are no stable-production guarantees for current previews. macOS signing/notarization and real-device portable capture/recovery remain tracked work. Current Rust builds use native Windows Credential Manager, macOS Keychain or Linux Secret Service; retained portable C# previews support session-only keys. Implementation presence does not prove every device or account policy works. Report the affected version and a sanitized synthetic reproduction whenever possible.

Legacy C# persistence uses exclusive random sibling temporary files, refuses linked destination/backup entries and checks directories below explicit application/OS storage roots. Configured OS/application roots can be redirected intentionally; they remain trusted deployment choices. These checks are not an OS sandbox against another actor concurrently replacing arbitrary same-user filesystem entries. Opening a supplied session also checks repair inputs before writing. Transcript TXT/JSON preserve literal source values; Markdown exports escape untrusted fields.
