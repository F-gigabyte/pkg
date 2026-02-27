use std::process::Command;

pub fn check_cmd(mut cmd: Command) -> Result<(), i32> {
    let status = cmd.output().map_err(|_| 0)?.status.code().ok_or(0)?;
    if  status == 0 {
        Ok(())
    } else {
        Err(status)
    }
}
