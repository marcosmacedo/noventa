use crate::actors::session_manager::{
    ClearSession, DeleteSessionValue, GetSessionValue, GetStatus, MarkAsModified, SessionManagerActor,
    SetPermanent, SetSessionValue,
};
use actix::Addr;
use actix_session::SessionStatus;
use pyo3::exceptions::{PyAttributeError, PyKeyError};
use pyo3::prelude::*;
use serde_json;

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
    #[getter]
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

    fn __getitem__(&self, py: Python, key: &str) -> PyResult<Py<PyAny>> {
        let msg = GetSessionValue {
            key: key.to_string(),
        };

        match futures::executor::block_on(self.session_manager.send(msg)) {
            Ok(Ok(Some(value))) => {
                let deserialized: serde_json::Value = serde_json::from_str(&value)
                    .map_err(|e| PyKeyError::new_err(e.to_string()))?;
                let py_obj = pythonize::pythonize(py, &deserialized)
                    .map_err(|e| PyKeyError::new_err(e.to_string()))?;
                Ok(py_obj.into())
            }
            Ok(Ok(None)) => Err(PyKeyError::new_err(key.to_string())),
            Ok(Err(e)) => Err(PyKeyError::new_err(e.to_string())),
            Err(e) => Err(PyKeyError::new_err(e.to_string())),
        }
    }

fn __setitem__(&mut self, py: Python, key: &str, value: Py<PyAny>) -> PyResult<()> {
    // In newer PyO3, bind() is used to get a Bound reference
    let bound_value = value.bind(py);

    let serialized_value: serde_json::Value = pythonize::depythonize(bound_value)
        .map_err(|e| PyKeyError::new_err(e.to_string()))?;
    let json_value = serde_json::to_string(&serialized_value)
        .map_err(|e| PyKeyError::new_err(e.to_string()))?;

    let msg = SetSessionValue {
        key: key.to_string(),
        value: json_value,
    };

    // Release the GIL before blocking
    let result = py.detach(|| {
        futures::executor::block_on(self.session_manager.send(msg))
    });

    match result {
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
    fn __contains__(&self, key: &str) -> PyResult<bool> {
        let msg = GetSessionValue {
            key: key.to_string(),
        };

        match futures::executor::block_on(self.session_manager.send(msg)) {
            Ok(Ok(Some(_))) => Ok(true),
            Ok(Ok(None)) => Ok(false),
            Ok(Err(_)) => Ok(false),
            Err(_) => Ok(false),
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

    #[pyo3(signature = (key, default = None))]
    fn get(&self, py: Python, key: &str, default: Option<Py<PyAny>>) -> PyResult<Py<PyAny>> {
        let msg = GetSessionValue {
            key: key.to_string(),
        };

        match futures::executor::block_on(self.session_manager.send(msg)) {
            Ok(Ok(Some(value))) => {
                let deserialized: serde_json::Value = serde_json::from_str(&value)
                    .map_err(|e| PyKeyError::new_err(e.to_string()))?;
                let py_obj = pythonize::pythonize(py, &deserialized)
                    .map_err(|e| PyKeyError::new_err(e.to_string()))?;
                Ok(py_obj.into())
            }
            Ok(Ok(None)) => Ok(default.unwrap_or_else(|| py.None())),
            Ok(Err(e)) => Err(PyKeyError::new_err(e.to_string())),
            Err(e) => Err(PyKeyError::new_err(e.to_string())),
        }
    }

    #[pyo3(signature = (key, default = None))]
    fn pop(&mut self, py: Python, key: &str, default: Option<Py<PyAny>>) -> PyResult<Py<PyAny>> {
        let get_msg = GetSessionValue {
            key: key.to_string(),
        };

        let value = match futures::executor::block_on(self.session_manager.send(get_msg)) {
            Ok(Ok(value)) => value,
            Ok(Err(e)) => return Err(PyKeyError::new_err(e.to_string())),
            Err(e) => return Err(PyKeyError::new_err(e.to_string())),
        };

        if let Some(val_str) = value {
            let del_msg = DeleteSessionValue {
                key: key.to_string(),
            };
            match futures::executor::block_on(self.session_manager.send(del_msg)) {
                Ok(Ok(_)) => {
                    let deserialized: serde_json::Value = serde_json::from_str(&val_str)
                        .map_err(|e| PyKeyError::new_err(e.to_string()))?;
                    let py_obj = pythonize::pythonize(py, &deserialized)
                        .map_err(|e| PyKeyError::new_err(e.to_string()))?;
                    return Ok(py_obj.into());
                }
                Ok(Err(e)) => return Err(PyKeyError::new_err(e.to_string())),
                Err(e) => return Err(PyKeyError::new_err(e.to_string())),
            }
        }

        Ok(default.unwrap_or_else(|| py.None()))
    }

    fn setdefault(&mut self, key: &str, default: &str) -> PyResult<String> {
        let get_msg = GetSessionValue {
            key: key.to_string(),
        };

        match futures::executor::block_on(self.session_manager.send(get_msg)) {
            Ok(Ok(Some(value))) => Ok(value),
            Ok(Ok(None)) => {
                let set_msg = SetSessionValue {
                    key: key.to_string(),
                    value: default.to_string(),
                };
                match futures::executor::block_on(self.session_manager.send(set_msg)) {
                    Ok(Ok(_)) => Ok(default.to_string()),
                    Ok(Err(e)) => Err(PyKeyError::new_err(e.to_string())),
                    Err(e) => Err(PyKeyError::new_err(e.to_string())),
                }
            }
            Ok(Err(e)) => Err(PyKeyError::new_err(e.to_string())),
            Err(e) => Err(PyKeyError::new_err(e.to_string())),
        }
    }
}