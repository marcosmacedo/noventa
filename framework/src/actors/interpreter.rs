use crate::actors::page_renderer::HttpRequestInfo;
use crate::components::Component;
use crate::config::CONFIG;
use crate::dto::python_request::PyRequest;
use actix::prelude::*;
use minijinja::Value;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};
use serde_json;
use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use std::path::Path;
use std::sync::Arc;

// Define the message for rendering a component
#[derive(Message, Clone)]
#[rtype(result = "Result<Value, Error>")]
pub struct ExecutePythonFunction {
    pub component_name: String,
    pub function_name: String,
    pub request: Arc<HttpRequestInfo>,
    pub args: Option<HashMap<String, Value>>,
}

use uuid::Uuid;

// Define the Python interpreter actor
pub struct PythonInterpreterActor {
    id: Uuid,
    modules: HashMap<String, Py<PyModule>>,
    components: Vec<Component>,
    db_instance: Option<Py<PyAny>>,
}

impl PythonInterpreterActor {
    pub fn new(components: Vec<Component>) -> Self {
        Self {
            id: Uuid::new_v4(),
            modules: HashMap::new(),
            components,
            db_instance: None,
        }
    }
}

impl Actor for PythonInterpreterActor {
    type Context = SyncContext<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        Python::attach(|py| {
            // 1️⃣ Add the 'web' folder to sys.path
            let sys = py.import("sys").unwrap();
            let path = sys.getattr("path").unwrap();
            path.call_method1("insert", (0, "../web")).unwrap();

            if let Some(db_url) = &CONFIG.database {
                match py.import("db") {
                    Ok(db_module) => match db_module.getattr("initialize_database") {
                        Ok(init_func) => match init_func.call1((db_url,)) {
                            Ok(db_instance) => {
                                self.db_instance = Some(db_instance.into());
                            }
                            Err(e) => {
                                log::error!("Failed to initialize database: {}", e);
                            }
                        },
                        Err(e) => {
                            log::error!("Failed to find initialize_database function: {}", e);
                        }
                    },
                    Err(e) => {
                        log::error!("Failed to load db.py: {}", e);
                    }
                }
            }

            // 2️⃣ Import each component by absolute path
            let importlib = py.import("importlib").unwrap();
            let import_module = importlib.getattr("import_module").unwrap();

            for component in &self.components {
                if let Some(logic_path) = &component.logic_path {
                    let module_path = match path_to_module(logic_path) {
                        Ok(path) => path,
                        Err(e) => {
                            log::error!("Failed to convert path to module for {}: {}", logic_path, e);
                            continue;
                        }
                    };

                    match import_module.call1((module_path,)) {
                        Ok(module) => {
                            self.modules.insert(
                                component.id.clone(),
                                module.downcast::<PyModule>().unwrap().clone().into(),
                            );
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
    type Result = Result<Value, Error>;

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

            let func = module
                .getattr(py, msg.function_name)
                .map_err(|e| pyerr_to_io_error(e, py))?;

            let py_request = Py::new(py, PyRequest { inner: msg.request }).unwrap();
            let py_args = PyDict::new(py);
            if let Some(args) = msg.args {
                for (key, value) in args {
                    let py_value = pythonize::pythonize(py, &value)
                        .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
                    py_args
                        .set_item(key, py_value)
                        .map_err(|e| pyerr_to_io_error(e, py))?;
                }
            }

            if let Some(db) = &self.db_instance {
                py_args
                    .set_item("db", db.as_ref())
                    .map_err(|e| pyerr_to_io_error(e, py))?;
            }

            let args = (py_request,);
            let result = func.call(py, args, Some(&py_args));

            let result = result.map_err(|e| pyerr_to_io_error(e, py))?;

            let py_any = result.bind(py);
            let serde_value: serde_json::Value = pythonize::depythonize(py_any)
                .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
            let value = Value::from_serialize(&serde_value);

            Ok(value)
        })
    }
}

fn pyerr_to_io_error(e: PyErr, py: Python) -> Error {
    let err_string = e.to_string();
    e.print(py);
    Error::new(ErrorKind::Other, err_string)
}

fn path_to_module(path_str: &str) -> Result<String, &'static str> {
    let _path = Path::new(path_str);

    // Strip the leading "./web/" from the path
    let cleaned_path_str = path_str.strip_prefix("./").unwrap_or(path_str);

    // Replace directory separators with dots and remove the .py extension
    let module_path = cleaned_path_str
        .strip_suffix(".py")
        .unwrap_or(cleaned_path_str)
        .replace("/", ".");

    Ok(module_path)
}