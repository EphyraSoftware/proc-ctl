use crate::ProcCtlError::ConfigurationError;
use crate::{Pid, ProcCtlResult};

pub(crate) trait MaybeHasPid {
    fn get_pid(&self) -> Option<Pid>;
}

pub(crate) fn resolve_pid(maybe_has_pid: &dyn MaybeHasPid) -> ProcCtlResult<Pid> {
    match &maybe_has_pid.get_pid() {
        Some(pid) => Ok(*pid),
        None => Err(ConfigurationError("unable to resolve a pid".to_string())),
    }
}
