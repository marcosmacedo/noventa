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
use serde::Serialize;

use crate::actors::session_manager::SessionManagerActor;
use actix::Addr;

#[derive(Debug, Clone, Serialize)]
pub struct PythonFunctionResult {
    pub context: Value,
}

#[derive(Message, Clone)]
#[rtype(result = "Result<PythonFunctionResult, Error>")]
pub struct ExecutePythonFunction {
    pub component_name: String,
    pub function_name: String,
    pub request: Arc<HttpRequestInfo>,
    pub args: Option<HashMap<String, Value>>,
    pub session_manager: Addr<SessionManagerActor>,
}

use uuid::Uuid;

#[derive(Message)]
#[rtype(result = "()")]
pub struct RescanComponents;

// Define the Python interpreter actor
pub struct PythonInterpreterActor {
    id: Uuid,
    modules: HashMap<String, Py<PyModule>>,
    components: Vec<Component>,
    db_instance: Option<Py<PyAny>>,
    dev_mode: bool,
}

impl PythonInterpreterActor {
    pub fn new(components: Vec<Component>, dev_mode: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            modules: HashMap::new(),
            components,
            db_instance: None,
            dev_mode,
        }
    }

    fn scan_components(&mut self) {
        if self.dev_mode {
            log::info!("Re-scanning components...");
            let components = crate::components::scan_components(std::path::Path::new("./components")).unwrap_or_else(|e| {
                log::error!("Failed to discover components: {}", e);
                vec![]
            });
            log::info!("Found {} components.", components.len());
            self.components = components;
        }
    }
}

impl Actor for PythonInterpreterActor {
    type Context = SyncContext<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        Python::attach(|py| {
            let sys = py.import("sys").unwrap();
            let path = sys.getattr("path").unwrap();
            path.call_method1("insert", (0, ".")).unwrap();

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

            if !self.dev_mode {
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
            }
        });
    }
}

// Define the handler for the ExecutePythonFunction message
impl Handler<ExecutePythonFunction> for PythonInterpreterActor {
    type Result = Result<PythonFunctionResult, Error>;

    fn handle(&mut self, msg: ExecutePythonFunction, _ctx: &mut Self::Context) -> Self::Result {
        log::info!(
            "Interpreter {} received request for component '{}'",
            self.id,
            msg.component_name
        );
        Python::attach(|py| {
            let module = if self.dev_mode {
                let component = self
                    .components
                    .iter()
                    .find(|c| c.id == msg.component_name)
                    .ok_or_else(|| Error::new(ErrorKind::NotFound, "Component not found in dev mode"))?;

                if let Some(logic_path) = &component.logic_path {
                    let module_path = path_to_module(logic_path)
                        .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;

                    let importlib = py
                        .import("importlib")
                        .map_err(|e| pyerr_to_io_error(e, py))?;
                    let import_module = importlib
                        .getattr("import_module")
                        .map_err(|e| pyerr_to_io_error(e, py))?;
                    let module = import_module
                        .call1((module_path,))
                        .map_err(|e| pyerr_to_io_error(e, py))?;

                    let reload = importlib
                        .getattr("reload")
                        .map_err(|e| pyerr_to_io_error(e, py))?;
                    reload.call1((module.clone(),)).map_err(|e| pyerr_to_io_error(e, py))?;

                    module
                        .downcast::<PyModule>()
                        .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?
                        .clone()
                        .into()
                } else {
                    return Err(Error::new(ErrorKind::NotFound, "Component logic not found"));
                }
            } else {
                self.modules
                    .get(&msg.component_name)
                    .map(|m| m.clone_ref(py))
                    .ok_or_else(|| Error::new(ErrorKind::NotFound, "Component not found"))?
            };

            let func = module
                .getattr(py, &msg.function_name)
                .map_err(|e| pyerr_to_io_error(e, py))?;

            let py_request = Py::new(py, PyRequest { inner: msg.request }).unwrap();
            let py_session_obj =
                Py::new(py, crate::dto::python_session::PySession::new(msg.session_manager)).unwrap();
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

            let args = if let Some(db) = &self.db_instance {
                (py_request, py_session_obj, db.clone_ref(py).into())
            } else {
                // This branch should ideally not be taken if db is always expected.
                // Consider how to handle the absence of a db instance.
                // For now, we'll pass PyNone.
                (py_request, py_session_obj, py.None())
            };
            let result = func.call(py, args, Some(&py_args));

            let result = result.map_err(|e| pyerr_to_io_error(e, py))?;

            let py_any = result.bind(py);
            let serde_value: serde_json::Value = pythonize::depythonize(py_any)
                .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
            let value = Value::from_serialize(&serde_value);

            Ok(PythonFunctionResult { context: value })
        })
    }
}

impl Handler<RescanComponents> for PythonInterpreterActor {
    type Result = ();

    fn handle(&mut self, _msg: RescanComponents, _ctx: &mut Self::Context) -> Self::Result {
        self.scan_components();
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