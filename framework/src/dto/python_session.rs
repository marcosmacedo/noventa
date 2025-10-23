use pyo3::prelude::*;
use std::collections::HashMap;

#[pyclass]
pub struct PySession {
    session: HashMap<String, String>,
}

impl PySession {
    pub fn new(session: HashMap<String, String>) -> Self {
        PySession { session }
    }

    pub fn get_session_state(&self) -> HashMap<String, String> {
        self.session.clone()
    }
}

#[pymethods]
impl PySession {
    fn __getitem__(&self, key: &str) -> PyResult<Option<String>> {
        Ok(self.session.get(key).cloned())
    }

    fn __setitem__(&mut self, key: &str, value: &str) -> PyResult<()> {
        self.session.insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn __delitem__(&mut self, key: &str) -> PyResult<()> {
        self.session.remove(key);
        Ok(())
    }

    fn get(&self, key: &str) -> PyResult<Option<String>> {
        self.__getitem__(key)
    }

    fn insert(&mut self, key: &str, value: &str) -> PyResult<()> {
        self.__setitem__(key, value)
    }

    fn remove(&mut self, key: &str) -> PyResult<()> {
        self.__delitem__(key)
    }

    fn clear(&mut self) {
        self.session.clear();
    }
}