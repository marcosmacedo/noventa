use crate::actors::session_manager::{
    ClearSession, DeleteSessionValue, GetSessionValue, GetStatus, MarkAsModified, SessionManagerActor,
    SetPermanent, SetSessionValue,
};
use actix::Addr;
use actix_session::SessionStatus;
use pyo3::exceptions::{PyAttributeError, PyKeyError};
use pyo3::prelude::*;

#[pyclass]
#[derive(Clone)]
pub struct PySession {
    session_manager: Addr<SessionManagerActor>,
}

impl PySession {
    pub fn new(session_manager: Addr<SessionManagerActor>) -> Self {
        PySession { session_manager }
    }
}

#[pymethods]
impl PySession {
    #[getter(new)]
    fn is_new(&self) -> PyResult<bool> {
        match futures::executor::block_on(self.session_manager.send(GetStatus)) {
            Ok(Ok(status)) => Ok(status == SessionStatus::Changed), // Simplified: actix-session doesn't expose "New" directly.
            Ok(Err(e)) => Err(PyAttributeError::new_err(e.to_string())),
            Err(e) => Err(PyAttributeError::new_err(e.to_string())),
        }
    }

    #[getter]
    fn modified(&self) -> PyResult<bool> {
        match futures::executor::block_on(self.session_manager.send(GetStatus)) {
            Ok(Ok(status)) => Ok(status == SessionStatus::Changed),
            Ok(Err(e)) => Err(PyAttributeError::new_err(e.to_string())),
            Err(e) => Err(PyAttributeError::new_err(e.to_string())),
        }
    }

    #[setter]
    fn set_modified(&self, value: bool) -> PyResult<()> {
        if value {
            match futures::executor::block_on(self.session_manager.send(MarkAsModified)) {
                Ok(Ok(_)) => Ok(()),
                Ok(Err(e)) => Err(PyAttributeError::new_err(e.to_string())),
                Err(e) => Err(PyAttributeError::new_err(e.to_string())),
            }
        } else {
            // Flask session doesn't allow setting modified to False,
            // but we can just do nothing.
            Ok(())
        }
    }

    #[getter]
    fn permanent(&self) -> PyResult<bool> {
        // This is a simplification. A full implementation would need to
        // inspect the cookie's expiration. For now, we'll assume
        // non-permanent unless explicitly set.
        Ok(false)
    }

    #[setter]
    fn set_permanent(&self, value: bool) -> PyResult<()> {
        let msg = SetPermanent { permanent: value };
        match futures::executor::block_on(self.session_manager.send(msg)) {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(PyAttributeError::new_err(e.to_string())),
            Err(e) => Err(PyAttributeError::new_err(e.to_string())),
        }
    }

    fn __getitem__(&self, key: &str) -> PyResult<Option<String>> {
        let msg = GetSessionValue {
            key: key.to_string(),
        };

        match futures::executor::block_on(self.session_manager.send(msg)) {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(e)) => Err(PyKeyError::new_err(e.to_string())),
            Err(e) => Err(PyKeyError::new_err(e.to_string())),
        }
    }

    fn __setitem__(&mut self, key: &str, value: &str) -> PyResult<()> {
        let msg = SetSessionValue {
            key: key.to_string(),
            value: value.to_string(),
        };

        match futures::executor::block_on(self.session_manager.send(msg)) {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(PyKeyError::new_err(e.to_string())),
            Err(e) => Err(PyKeyError::new_err(e.to_string())),
        }
    }

    fn __delitem__(&mut self, key: &str) -> PyResult<()> {
        let msg = DeleteSessionValue {
            key: key.to_string(),
        };

        match futures::executor::block_on(self.session_manager.send(msg)) {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(PyKeyError::new_err(e.to_string())),
            Err(e) => Err(PyKeyError::new_err(e.to_string())),
        }
    }

    fn clear(&mut self) -> PyResult<()> {
        let msg = ClearSession;

        match futures::executor::block_on(self.session_manager.send(msg)) {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(PyKeyError::new_err(e.to_string())),
            Err(e) => Err(PyKeyError::new_err(e.to_string())),
        }
    }
}