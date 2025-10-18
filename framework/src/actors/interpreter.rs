use crate::actors::page_renderer::HttpRequestInfo;
use crate::components::Component;
use crate::dto::python_request::PyRequest;
use actix::prelude::*;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule, PyTuple};
use serde_json::Value;
use std::collections::HashMap;
use std::ffi::CString;
use std::io::{Error, ErrorKind};
use std::sync::Arc;

// Define the message for rendering a component
#[derive(Message, Clone)]
#[rtype(result = "Result<HashMap<String, Value>, Error>")]
pub struct ExecutePythonFunction {
    pub component_name: String,
    pub function_name: String,
    pub request: Arc<HttpRequestInfo>,
    pub args: Option<HashMap<String, String>>,
}

use uuid::Uuid;

// Define the Python interpreter actor
pub struct PythonInterpreterActor {
    id: Uuid,
    modules: HashMap<String, Py<PyModule>>,
    components: Vec<Component>,
}

impl PythonInterpreterActor {
    pub fn new(components: Vec<Component>) -> Self {
        Self {
            id: Uuid::new_v4(),
            modules: HashMap::new(),
            components,
        }
    }
}

impl Actor for PythonInterpreterActor {
    type Context = SyncContext<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        Python::attach(|py| {
            for component in &self.components {
                if let Some(code_path) = &component.code_path {
                    let py_path = std::path::Path::new(code_path);

                    if !py_path.exists() {
                        log::error!(
                            "Component code file does not exist: {}",
                            py_path.to_string_lossy()
                        );
                        continue;
                    }

                    let py_path_str = py_path.to_str().unwrap();

                    let code_string = match std::fs::read_to_string(&py_path) {
                        Ok(s) => s,
                        Err(e) => {
                            log::error!("Failed to read component file {}: {}", py_path_str, e);
                            continue;
                        }
                    };

                    let code_cstr = CString::new(code_string.as_str()).unwrap();
                    let path_cstr = CString::new(py_path_str).unwrap();
                    let name_cstr = CString::new(component.id.as_str()).unwrap();

                    match PyModule::from_code(py, &code_cstr, &path_cstr, &name_cstr) {
                        Ok(module) => {
                            self.modules.insert(component.id.clone(), module.into());
                        }
                        Err(e) => {
                            log::error!("Failed to load component {}: {}", component.id, e);
                        }
                    }
                }
            }
        });
    }
}

// Define the handler for the ExecutePythonFunction message
impl Handler<ExecutePythonFunction> for PythonInterpreterActor {
    type Result = Result<HashMap<String, Value>, Error>;

    fn handle(&mut self, msg: ExecutePythonFunction, _ctx: &mut Self::Context) -> Self::Result {
        log::info!(
            "Interpreter {} received request for component '{}'",
            self.id,
            msg.component_name
        );
        Python::attach(|py| {
            let module = self
                .modules
                .get(&msg.component_name)
                .ok_or_else(|| Error::new(ErrorKind::NotFound, "Component not found"))?;

            let func = module.getattr(py, msg.function_name)
                .map_err(|e| pyerr_to_io_error(e, py))?;

            let py_request = Py::new(py, PyRequest { inner: msg.request }).unwrap();
            let result = if let Some(args) = msg.args {
                let py_args = PyDict::new(py);
                for (key, value) in args {
                    py_args.set_item(key, value).map_err(|e| pyerr_to_io_error(e, py))?;
                }
                let args = (py_request,);
                func.call1(py, args)
            } else {
                let args = (py_request,);
                func.call1(py, args)
            };

            let result = result.map_err(|e| pyerr_to_io_error(e, py))?;

            let dict = result
                .bind(py)
                .downcast::<PyDict>()
                .map_err(|e| pyerr_to_io_error(e.into(), py))?;

            let mut context: HashMap<String, Value> = HashMap::new();
            
            for (key, value) in dict {
                let key: String = key.extract().map_err(|e| pyerr_to_io_error(e, py))?;
                let py_string = value.str().map_err(|e| pyerr_to_io_error(e, py))?;
                let value_str: &str = py_string
                    .to_str()
                    .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
                let value: Value = serde_json::from_str(value_str)
                    .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
                context.insert(key, value);
            }
            Ok(context)
        })
    }
}

fn pyerr_to_io_error(e: PyErr, py: Python) -> Error {
    let err_string = e.to_string();
    e.print(py);
    Error::new(ErrorKind::Other, err_string)
}