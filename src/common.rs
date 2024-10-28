#[cfg(any(target_os = "linux", feature = "proc"))]
pub(crate) trait MaybeHasPid {
    fn get_pid(&self) -> Option<crate::Pid>;
}

#[cfg(any(target_os = "linux", feature = "proc"))]
pub(crate) fn resolve_pid(maybe_has_pid: &dyn MaybeHasPid) -> crate::ProcCtlResult<crate::Pid> {
    match &maybe_has_pid.get_pid() {
        Some(pid) => Ok(*pid),
        None => Err(crate::ProcCtlError::ConfigurationError(
            "unable to resolve a pid".to_string(),
        )),
    }
}
