use crate::actors::page_renderer::{FileData, HttpRequestInfo};
use pyo3::{prelude::*, exceptions::PyNotImplementedError};
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
                scheme: "".to_string(),
                host: "".to_string(),
                remote_addr: None,
                url: "".to_string(),
                base_url: "".to_string(),
                host_url: "".to_string(),
                url_root: "".to_string(),
                full_path: "".to_string(),
                query_string: Vec::new(),
                cookies: std::collections::HashMap::new(),
                user_agent: None,
                content_type: None,
                content_length: None,
                is_secure: false,
                is_xhr: false,
                accept_charsets: Vec::new(),
                accept_encodings: Vec::new(),
                accept_languages: Vec::new(),
                accept_mimetypes: Vec::new(),
                access_route: Vec::new(),
                authorization: None,
                cache_control: None,
                content_encoding: None,
                content_md5: None,
                date: None,
                if_match: Vec::new(),
                if_modified_since: None,
                if_none_match: Vec::new(),
                if_range: None,
                if_unmodified_since: None,
                max_forwards: None,
                pragma: None,
                range: None,
                referrer: None,
                remote_user: None,
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
    #[getter]
    fn scheme(&self) -> &str {
        &self.inner.scheme
    }

    #[getter]
    fn host(&self) -> &str {
        &self.inner.host
    }

    #[getter]
    fn remote_addr(&self) -> Option<&str> {
        self.inner.remote_addr.as_deref()
    }

    #[getter]
    fn url(&self) -> &str {
        &self.inner.url
    }

    #[getter]
    fn base_url(&self) -> &str {
        &self.inner.base_url
    }

    #[getter]
    fn host_url(&self) -> &str {
        &self.inner.host_url
    }

    #[getter]
    fn url_root(&self) -> &str {
        &self.inner.url_root
    }

    #[getter]
    fn full_path(&self) -> &str {
        &self.inner.full_path
    }

    #[getter]
    fn query_string(&self) -> &[u8] {
        &self.inner.query_string
    }

    #[getter]
    fn user_agent(&self) -> Option<&str> {
        self.inner.user_agent.as_deref()
    }

    #[getter]
    fn content_type(&self) -> Option<&str> {
        self.inner.content_type.as_deref()
    }

    #[getter]
    fn content_length(&self) -> Option<usize> {
        self.inner.content_length
    }

    #[getter]
    fn is_secure(&self) -> bool {
        self.inner.is_secure
    }

    #[getter]
    fn is_xhr(&self) -> bool {
        self.inner.is_xhr
    }

    #[getter]
    fn accept_charsets(&self) -> Vec<String> {
        self.inner.accept_charsets.clone()
    }

    #[getter]
    fn accept_encodings(&self) -> Vec<String> {
        self.inner.accept_encodings.clone()
    }

    #[getter]
    fn accept_languages(&self) -> Vec<String> {
        self.inner.accept_languages.clone()
    }

    #[getter]
    fn accept_mimetypes(&self) -> Vec<String> {
        self.inner.accept_mimetypes.clone()
    }

    #[getter]
    fn access_route(&self) -> Vec<String> {
        self.inner.access_route.clone()
    }

    #[getter]
    fn authorization(&self) -> Option<String> {
        self.inner.authorization.clone()
    }

    #[getter]
    fn cache_control(&self) -> Option<String> {
        self.inner.cache_control.clone()
    }

    #[getter]
    fn content_encoding(&self) -> Option<String> {
        self.inner.content_encoding.clone()
    }

    #[getter]
    fn date(&self) -> Option<String> {
        self.inner.date.clone()
    }

    #[getter]
    fn if_match(&self) -> Vec<String> {
        self.inner.if_match.clone()
    }

    #[getter]
    fn if_modified_since(&self) -> Option<String> {
        self.inner.if_modified_since.clone()
    }

    #[getter]
    fn if_none_match(&self) -> Vec<String> {
        self.inner.if_none_match.clone()
    }

    #[getter]
    fn if_range(&self) -> Option<String> {
        self.inner.if_range.clone()
    }

    #[getter]
    fn if_unmodified_since(&self) -> Option<String> {
        self.inner.if_unmodified_since.clone()
    }

    #[getter]
    fn max_forwards(&self) -> Option<String> {
        self.inner.max_forwards.clone()
    }

    #[getter]
    fn pragma(&self) -> Option<String> {
        self.inner.pragma.clone()
    }

    #[getter]
    fn range(&self) -> Option<String> {
        self.inner.range.clone()
    }

    #[getter]
    fn referrer(&self) -> Option<String> {
        self.inner.referrer.clone()
    }

    #[getter]
    fn remote_user(&self) -> Option<String> {
        self.inner.remote_user.clone()
    }

    #[getter]
    fn charset(&self) -> String {
        self.inner.content_type.as_deref().unwrap_or("").split(';').nth(1).and_then(|s| s.trim().split('=').nth(1)).unwrap_or("").to_string()
    }

    #[getter]
    fn mimetype(&self) -> String {
        self.inner.content_type.as_deref().unwrap_or("").split(';').next().unwrap_or("").to_string()
    }

    #[getter]
    fn mimetype_params(&self, py: Python) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        if let Some(content_type) = &self.inner.content_type {
            for part in content_type.split(';').skip(1) {
                let mut params = part.splitn(2, '=');
                if let (Some(key), Some(value)) = (params.next(), params.next()) {
                    dict.set_item(key.trim(), value.trim())?;
                }
            }
        }
        Ok(dict.into())
    }

    fn data(&self, py: Python) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        for (key, value) in &self.inner.form_data {
            dict.set_item(key, to_pyobject(py, value)?)?;
        }
        Ok(dict.into())
    }

    #[getter]
    fn values(&self, py: Python) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        for (key, value) in &self.inner.query_params {
            dict.set_item(key, value)?;
        }
        for (key, value) in &self.inner.form_data {
            dict.set_item(key, to_pyobject(py, value)?)?;
        }
        Ok(dict.into())
    }

    #[getter]
    fn want_form_data_parsed(&self) -> bool {
        self.inner.method == "POST" || self.inner.method == "PUT" || self.inner.method == "PATCH"
    }

    #[getter]
    fn is_json(&self) -> bool {
        if let Some(content_type) = &self.inner.content_type {
            let mimetype = content_type.split(';').next().unwrap_or("").trim();
            mimetype == "application/json" || mimetype.ends_with("+json")
        } else {
            false
        }
    }
    #[getter]
    fn disable_data_descriptor(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Notice: This attribute is not implemented on purpose. Please find a workaround coding in other way"))
    }

    #[getter]
    fn encoding_errors(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Notice: This attribute is not implemented on purpose. Please find a workaround coding in other way"))
    }

    #[getter]
    fn endpoint(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Notice: This attribute is not implemented on purpose. Please find a workaround coding in other way"))
    }

    #[getter]
    fn input_stream(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Notice: This attribute is not implemented on purpose. Please find a workaround coding in other way"))
    }

    #[getter]
    fn is_multiprocess(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Notice: This attribute is not implemented on purpose. Please find a workaround coding in other way"))
    }

    #[getter]
    fn is_multithread(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Notice: This attribute is not implemented on purpose. Please find a workaround coding in other way"))
    }

    #[getter]
    fn is_run_once(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Notice: This attribute is not implemented on purpose. Please find a workaround coding in other way"))
    }

    #[getter]
    fn max_content_length(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Notice: This attribute is not implemented on purpose. Please find a workaround coding in other way"))
    }

    #[getter]
    fn max_form_memory_size(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Notice: This attribute is not implemented on purpose. Please find a workaround coding in other way"))
    }

    #[getter]
    fn module(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Notice: This attribute is not implemented on purpose. Please find a workaround coding in other way"))
    }

    #[getter]
    fn routing_exception(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Notice: This attribute is not implemented on purpose. Please find a workaround coding in other way"))
    }

    #[getter]
    fn script_root(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Notice: This attribute is not implemented on purpose. Please find a workaround coding in other way"))
    }

    #[getter]
    fn stream(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Notice: This attribute is not implemented on purpose. Please find a workaround coding in other way"))
    }

    #[getter]
    fn trusted_hosts(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Notice: This attribute is not implemented on purpose. Please find a workaround coding in other way"))
    }

    #[getter]
    fn url_charset(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Notice: This attribute is not implemented on purpose. Please find a workaround coding in other way"))
    }

    #[getter]
    fn url_rule(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Notice: This attribute is not implemented on purpose. Please find a workaround coding in other way"))
    }

    #[getter]
    fn view_args(&self, py: Python) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        for (key, value) in &self.inner.path_params {
            dict.set_item(key, value)?;
        }
        Ok(dict.into())
    }
    fn close(&self) -> PyResult<()> {
        Ok(())
    }

    fn get_data(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Notice: This attribute is not implemented on purpose. Please find a workaround coding in other way"))
    }

    fn get_json(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("Notice: This attribute is not implemented on purpose. Please find a workaround coding in other way"))
    }
}