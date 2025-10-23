use crate::actors::page_renderer::{FileData, HttpRequestInfo};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_pyobject::to_pyobject;
use std::io::Write;
use std::sync::Arc;

#[pyclass]
pub struct PyFileStorage {
    #[pyo3(get, set)]
    filename: String,
    #[pyo3(get, set)]
    content_type: String,
    #[pyo3(get, set)]
    headers: Py<PyDict>,
    data: Arc<FileData>,
}

#[pymethods]
impl PyFileStorage {
    #[new]
    fn new(filename: String, content_type: String, headers: Py<PyDict>) -> Self {
        PyFileStorage {
            filename,
            content_type,
            headers,
            data: Arc::new(FileData::InMemory(Vec::new())),
        }
    }

    fn save(&self, destination: String) -> PyResult<()> {
        let mut file = std::fs::File::create(&destination)?;
        match &*self.data {
            FileData::InMemory(bytes) => {
                file.write_all(bytes)?;
            }
            FileData::OnDisk(path) => {
                std::fs::copy(path, destination)?;
            }
        }
        Ok(())
    }

    fn read(&self) -> PyResult<Vec<u8>> {
        match &*self.data {
            FileData::InMemory(bytes) => Ok(bytes.clone()),
            FileData::OnDisk(path) => Ok(std::fs::read(path)?),
        }
    }

    fn stream<'a>(&self, py: Python<'a>) -> PyResult<Py<pyo3::types::PyBytes>> {
        let bytes = self.read()?;
        Ok(pyo3::types::PyBytes::new(py, &bytes).into())
    }
}

impl Drop for PyFileStorage {
    fn drop(&mut self) {
        if let FileData::OnDisk(path) = &*self.data {
            if let Err(e) = std::fs::remove_file(path) {
                log::error!("Failed to delete temporary file: {}", e);
            }
        }
    }
}

#[pyclass]
#[derive(Clone)]
pub struct PyRequest {
    pub inner: Arc<HttpRequestInfo>,
}

#[pymethods]
impl PyRequest {
    #[new]
    fn new() -> Self {
        PyRequest {
            inner: Arc::new(HttpRequestInfo {
                path: "".to_string(),
                method: "".to_string(),
                headers: std::collections::HashMap::new(),
                form_data: serde_json::Map::new(),
                files: std::collections::HashMap::new(),
                query_params: std::collections::HashMap::new(),
                path_params: std::collections::HashMap::new(),
            }),
        }
    }

    #[getter]
    fn path(&self) -> &str {
        &self.inner.path
    }

    #[getter]
    fn method(&self) -> &str {
        &self.inner.method
    }

    #[getter]
    fn args(&self, py: Python) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        for (key, value) in &self.inner.query_params {
            dict.set_item(key, value)?;
        }
        Ok(dict.into())
    }

    #[getter]
    fn form(&self, py: Python) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        for (key, value) in &self.inner.form_data {
            dict.set_item(key, to_pyobject(py, value)?)?;
        }
        Ok(dict.into())
    }

    #[getter]
    fn files(&self, py: Python) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        for (key, value) in &self.inner.files {
            let headers_dict = PyDict::new(py);
            for (h_key, h_value) in &value.headers {
                headers_dict.set_item(h_key, h_value)?;
            }
            let file_storage = Py::new(
                py,
                PyFileStorage {
                    filename: value.filename.clone(),
                    content_type: value.content_type.clone(),
                    headers: headers_dict.into(),
                    data: Arc::new(value.data.clone()),
                },
            )?;
            dict.set_item(key, file_storage)?;
        }
        Ok(dict.into())
    }

    #[getter]
    fn headers(&self, py: Python) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        for (key, value) in &self.inner.headers {
            dict.set_item(key, value)?;
        }
        Ok(dict.into())
    }

    #[getter]
    fn cookies(&self, py: Python) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        if let Some(cookie_header) = self.inner.headers.get("cookie") {
            for cookie in cookie_header.split(';') {
                let mut parts = cookie.splitn(2, '=');
                if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                    dict.set_item(key.trim(), value.trim())?;
                }
            }
        }
        Ok(dict.into())
    }
}