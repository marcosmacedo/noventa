use crate::actors::page_renderer::HttpRequestInfo;
use crate::config::CONFIG;
use crate::dto::python_request::PyRequest;
use actix::prelude::*;
use minijinja::Value;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};
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
pub struct ExecuteFunction {
    pub module_path: String,
    pub function_name: String,
    pub request: Arc<HttpRequestInfo>,
    pub args: Option<HashMap<String, Value>>,
    pub session_manager: Addr<SessionManagerActor>,
}

use uuid::Uuid;

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct ReloadInterpreter;


// Define the Python interpreter actor
pub struct PythonInterpreterActor {
    id: Uuid,
    modules: HashMap<String, Py<PyModule>>,
    db_instance: Option<Py<PyAny>>,
    dev_mode: bool,
}

impl PythonInterpreterActor {
    pub fn new(dev_mode: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            modules: HashMap::new(),
            db_instance: None,
            dev_mode,
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
                                log::error!("Oh no! We couldn't initialize the database. Your `db.py` script ran into an error: {}", e);
                            }
                        },
                        Err(e) => {
                            log::error!("We couldn't find the `initialize_database` function in your `db.py` file: {}. Did you remember to define it?", e);
                        }
                    },
                    Err(e) => {
                        log::error!("We couldn't load `db.py`: {}. Please make sure the file exists and has the correct permissions.", e);
                    }
                }
            }

        });
    }
}

// Define the handler for the ExecuteFunction message
impl Handler<ExecuteFunction> for PythonInterpreterActor {
    type Result = Result<PythonFunctionResult, Error>;

    fn handle(&mut self, msg: ExecuteFunction, _ctx: &mut Self::Context) -> Self::Result {
        log::trace!(
            "Interpreter {} received request for module '{}' and function '{}'",
            self.id,
            msg.module_path,
            msg.function_name
        );

        let py_request = PyRequest { inner: msg.request };
        let py_session = crate::dto::python_session::PySession::new(msg.session_manager);

        let result_value: serde_json::Value = Python::attach(|py| {
            let module = if self.dev_mode {
                let importlib = py.import("importlib").map_err(|e| pyerr_to_io_error(e, py))?;
                let import_module = importlib.getattr("import_module").map_err(|e| pyerr_to_io_error(e, py))?;
                let module = import_module.call1((&msg.module_path,)).map_err(|e| pyerr_to_io_error(e, py))?;
                let reload = importlib.getattr("reload").map_err(|e| pyerr_to_io_error(e, py))?;
                reload.call1((module.clone(),)).map_err(|e| pyerr_to_io_error(e, py))?;
                module.downcast::<PyModule>().map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?.clone().into()
            } else {
                self.modules
                    .get(&msg.module_path)
                    .map(|m| m.clone_ref(py))
                    .ok_or_else(|| Error::new(ErrorKind::NotFound, "Module not found"))?
            };

            let func = module.getattr(py, &msg.function_name).map_err(|e| pyerr_to_io_error(e, py))?;

            let py_request_obj = Py::new(py, py_request).unwrap();
            let py_session_obj = Py::new(py, py_session).unwrap();

            let py_args = PyDict::new(py);
            if let Some(args) = msg.args {
                for (key, value) in args {
                    let py_value = pythonize::pythonize(py, &value)
                        .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
                    py_args.set_item(key, py_value).map_err(|e| pyerr_to_io_error(e, py))?;
                }
            }

            let args = if let Some(db) = &self.db_instance {
                (py_request_obj, py_session_obj, db.clone_ref(py).into())
            } else {
                (py_request_obj, py_session_obj, py.None())
            };

            let result = func.call(py, args, Some(&py_args)).map_err(|e| pyerr_to_io_error(e, py))?;
            let py_any = result.bind(py);

            pythonize::depythonize(py_any).map_err(|e| Error::new(ErrorKind::Other, e.to_string()))
        })?;

        let value = Value::from_serialize(&result_value);
        Ok(PythonFunctionResult { context: value })
    }
}


impl Handler<ReloadInterpreter> for PythonInterpreterActor {
    type Result = ();

    fn handle(&mut self, _msg: ReloadInterpreter, ctx: &mut Self::Context) -> Self::Result {
        log::debug!("Interpreter {} received reload request", self.id);
        self.started(ctx);
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