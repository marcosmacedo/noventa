use crate::actors::page_renderer::HttpRequestInfo;
use crate::config::CONFIG;
use crate::dto::python_request::PyRequest;
use actix::prelude::*;
use minijinja::Value;
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyDict, PyModule};
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::Arc;
use std::fmt;

// Define the message for rendering a component
use serde::{Deserialize, Serialize};

use crate::actors::session_manager::SessionManagerActor;
use actix::Addr;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PythonError {
    pub message: String,
    pub traceback: String,
    pub line_number: Option<usize>,
    pub filename: Option<String>,
    pub source_code: Option<String>,
}

impl fmt::Display for PythonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PythonError {}

#[derive(Debug, Clone, Serialize)]
pub struct PythonFunctionResult {
    pub context: Value,
}

#[derive(Message, Clone)]
#[rtype(result = "Result<PythonFunctionResult, PythonError>")]
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
                let db_code = CString::new(crate::scripts::python_embed::DB_PY).unwrap();
                let db_filename = CString::new("db.py").unwrap();
                let db_module_name = CString::new("db").unwrap();
                match PyModule::from_code(py, &db_code, &db_filename, &db_module_name) {
                    Ok(db_module) => match db_module.getattr("initialize_database") {
                        Ok(init_func) => match init_func.call1((db_url,)) {
                            Ok(db_instance) => {
                                self.db_instance = Some(db_instance.into());
                            }
                            Err(e) => {
                                log::error!("Failed to initialize the database from embedded script: {}", e);
                            }
                        },
                        Err(e) => {
                            log::error!("Could not find `initialize_database` in embedded db.py: {}", e);
                        }
                    },
                    Err(e) => {
                        log::error!("Failed to load embedded db.py module: {}", e);
                    }
                }
            }

        });
    }
}

// Define the handler for the ExecuteFunction message
impl Handler<ExecuteFunction> for PythonInterpreterActor {
    type Result = Result<PythonFunctionResult, PythonError>;

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
                let importlib = py.import("importlib").map_err(|e| pyerr_to_pyerror(e, py))?;
                let import_module = importlib.getattr("import_module").map_err(|e| pyerr_to_pyerror(e, py))?;
                let module = import_module.call1((&msg.module_path,)).map_err(|e| pyerr_to_pyerror(e, py))?;
                let reload = importlib.getattr("reload").map_err(|e| pyerr_to_pyerror(e, py))?;
                reload.call1((module.clone(),)).map_err(|e| pyerr_to_pyerror(e, py))?;
                module.downcast::<PyModule>().map_err(|e| PythonError {
                    message: e.to_string(),
                    traceback: "".to_string(),
                    line_number: None,
                    filename: None,
                    source_code: None,
                })?.clone().into()
            } else {
                self.modules
                    .get(&msg.module_path)
                    .map(|m| m.clone_ref(py))
                    .ok_or_else(|| PythonError {
                        message: "Module not found".to_string(),
                        traceback: "".to_string(),
                        line_number: None,
                        filename: None,
                        source_code: None,
                    })?
            };

            let func = module.getattr(py, &msg.function_name).map_err(|e| pyerr_to_pyerror(e, py))?;

            let py_request_obj = Py::new(py, py_request).unwrap();
            let py_session_obj = Py::new(py, py_session).unwrap();

            let py_args = PyDict::new(py);
            if let Some(args) = msg.args {
                for (key, value) in args {
                    let py_value = pythonize::pythonize(py, &value)
                        .map_err(|e| PythonError {
                            message: e.to_string(),
                            traceback: "".to_string(),
                            line_number: None,
                            filename: None,
                            source_code: None,
                        })?;
                    py_args.set_item(key, py_value).map_err(|e| pyerr_to_pyerror(e, py))?;
                }
            }

            let db_arg = self.db_instance.as_ref().map_or(py.None(), |db| db.clone_ref(py).into());

            // Load the embedded Python utils from the new path
            let utils_code = CString::new(crate::scripts::python_embed::UTILS_PY).unwrap();
            let utils_filename = CString::new("utils.py").unwrap();
            let utils_module_name = CString::new("utils").unwrap();
            let utils_module = PyModule::from_code(py, &utils_code, &utils_filename, &utils_module_name)
                .map_err(|e| pyerr_to_pyerror(e, py))?;
            let wrapper_func = utils_module.getattr("call_user_function")
                .map_err(|e| pyerr_to_pyerror(e, py))?;

            // The user's function and its arguments are passed to the wrapper
            let args_to_wrapper = (func, py_request_obj, py_session_obj, db_arg);
            let result = wrapper_func.call(args_to_wrapper, Some(&py_args)).map_err(|e| pyerr_to_pyerror(e, py))?;
            
            let py_any = result;
            pythonize::depythonize(&py_any).map_err(|e| PythonError {
                message: e.to_string(),
                traceback: "".to_string(),
                line_number: None,
                filename: None,
                source_code: None,
            })
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

fn pyerr_to_pyerror(e: PyErr, py: Python) -> PythonError {
    let mut filename = None;
    let mut line_number = None;
    let mut source_code = None;
    let mut traceback_str = "No traceback available".to_string();

    let result: PyResult<()> = (|| {
        let traceback_module = py.import("traceback")?;
        let tb_obj = e.traceback(py).map_or(py.None(), |tb| tb.into());

        // Full formatted traceback string (for logs, debugging, etc.)
        let tb_list = traceback_module.call_method1(
            "format_exception",
            (e.get_type(py), e.value(py), tb_obj),
        )?;
        traceback_str = tb_list.extract::<Vec<String>>()?.join("");

        // Extract structured traceback info (list of FrameSummary)
        if let Some(tb) = e.traceback(py) {
            let frames = traceback_module.call_method1("extract_tb", (tb,))?;
            let frames_len: usize = frames.len()?;

            // Skip the first 2 frames (your wrapper)
            if frames_len > 2 {
                let user_frame = frames.get_item(frames_len - 1)?; // last frame (innermost user error)
                let fname: String = user_frame.getattr("filename")?.extract()?;
                let lineno: usize = user_frame.getattr("lineno")?.extract()?;
                let _func: String = user_frame.getattr("name")?.extract()?;

                filename = Some(fname.clone());
                line_number = Some(lineno);

                // Optional: extract nearby source code context
                if let Ok(contents) = std::fs::read_to_string(&fname) {
                    let lines: Vec<_> = contents.lines().collect();
                    let start = (lineno.saturating_sub(6)).max(1) - 1;
                    let end = (lineno + 5).min(lines.len());
                    source_code = Some(lines[start..end].join("\n"));
                }
            } else {
                log::debug!("Traceback has fewer than 3 frames; cannot skip wrapper frames.");
            }
        }
        Ok(())
    })();

    if let Err(e) = result {
        log::error!("Error getting traceback: {}", e);
    }

    PythonError {
        message: e.to_string(),
        traceback: traceback_str,
        line_number,
        filename,
        source_code,
    }
}

