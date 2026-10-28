//! Long-running work shares one admission policy. A lease is held until its work
//! really finishes, including cancellation and unwinding, rather than until spawn.
use std::sync::{Arc, Mutex, TryLockError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Recording,
    Analysis,
    Speakers,
    Index,
    Export,
    Mutation,
    Maintenance,
    Installation,
    Selection,
    /// Live notes write into one session folder; archive and installation must wait for them.
    LiveNotes,
}
impl Kind {
    /// Identify kinds requiring every other admitted operation to have finished.
    pub(crate) fn exclusive(self) -> bool {
        matches!(
            self,
            Self::Maintenance | Self::Installation | Self::Selection
        )
    }
    /// Reject duplicate kinds and any pairing containing an exclusive operation.
    fn conflicts(self, other: Self) -> bool {
        self == other || self.exclusive() || other.exclusive()
    }
}

const BUSY: &str = "Another operation is still running. Wait for it to finish or cancel it first. / Ein anderer Vorgang läuft noch. Bitte zuerst abschließen oder abbrechen.";

#[derive(Default)]
pub(crate) struct Gate(Arc<Mutex<Vec<Kind>>>);
pub(crate) struct Lease {
    gate: Arc<Mutex<Vec<Kind>>>,
    kind: Kind,
}
impl Gate {
    /// Reserve admission without waiting for mutex contention or conflicting work. The returned
    /// lease must span actual work, cancellation and cleanup.
    pub(crate) fn try_acquire(&self, kind: Kind) -> Result<Lease, String> {
        let mut active = self.0.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => BUSY.to_owned(),
            TryLockError::Poisoned(_) => "Operation control is unavailable; saved work is preserved. / Vorgangssteuerung ist gesperrt; gesicherte Daten bleiben erhalten.".into(),
        })?;
        if active.iter().any(|other| kind.conflicts(*other)) {
            return Err(BUSY.into());
        }
        active.push(kind);
        Ok(Lease {
            gate: self.0.clone(),
            kind,
        })
    }
}
impl Drop for Lease {
    /// Release only this lease kind while keeping other admitted operations reserved.
    fn drop(&mut self) {
        if let Ok(mut active) = self.gate.lock() {
            if let Some(index) = active.iter().position(|kind| *kind == self.kind) {
                active.swap_remove(index);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify synthetic leases exclude all exclusive work until the held operation is dropped.
    #[test]
    fn destructive_work_refuses_every_active_kind_and_releases_on_exit() {
        for kind in [
            Kind::Recording,
            Kind::Analysis,
            Kind::Speakers,
            Kind::Index,
            Kind::Export,
            Kind::Mutation,
            Kind::Maintenance,
            Kind::Installation,
            Kind::Selection,
            Kind::LiveNotes,
        ] {
            let gate = Gate::default();
            let lease = gate.try_acquire(kind).unwrap();
            for exclusive in [Kind::Maintenance, Kind::Installation, Kind::Selection] {
                assert!(
                    gate.try_acquire(exclusive).is_err(),
                    "{kind:?}/{exclusive:?}"
                );
            }
            drop(lease);
            assert!(gate.try_acquire(Kind::Maintenance).is_ok());
        }
    }

    /// Verify distinct nonexclusive work coexists while duplicate kinds and mutex contention
    /// return busy.
    #[test]
    fn independent_non_destructive_work_can_coexist_and_contention_returns_busy() {
        let gate = Gate::default();
        let recording = gate.try_acquire(Kind::Recording).unwrap();
        let analysis = gate.try_acquire(Kind::Analysis).unwrap();
        let speakers = gate.try_acquire(Kind::Speakers).unwrap();
        let index = gate.try_acquire(Kind::Index).unwrap();
        assert!(gate.try_acquire(Kind::Recording).is_err());
        let locked = gate.0.lock().unwrap();
        assert!(gate
            .try_acquire(Kind::Maintenance)
            .err()
            .unwrap()
            .contains("running"));
        drop(locked);
        drop((recording, analysis, speakers, index));
        assert!(gate.try_acquire(Kind::Installation).is_ok());
    }

    /// Verify unwinding releases synthetic analysis admission without releasing concurrent
    /// recording admission.
    #[test]
    fn unwinding_releases_only_the_owned_operation() {
        let gate = Gate::default();
        let recording = gate.try_acquire(Kind::Recording).unwrap();
        let result = std::panic::catch_unwind(|| {
            let _analysis = gate.try_acquire(Kind::Analysis).unwrap();
            panic!("synthetic worker failure");
        });
        assert!(result.is_err());
        assert!(gate.try_acquire(Kind::Analysis).is_ok());
        assert!(gate.try_acquire(Kind::Maintenance).is_err());
        drop(recording);
        assert!(gate.try_acquire(Kind::Maintenance).is_ok());
    }
}
