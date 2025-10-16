use crate::components::Component;
use actix::prelude::*;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};
use serde_json::Value;
use std::collections::HashMap;
use std::ffi::CString;
use std::io::{Error, ErrorKind};

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct LoadComponents {
    pub components: HashMap<String, Component>,
}

// Define the message for rendering a component
#[derive(Message)]
#[rtype(result = "Result<HashMap<String, Value>, Error>")]
pub struct RenderComponent {
    pub name: String,
}

// Define the Python interpreter actor
pub struct PythonInterpreterActor {
    modules: HashMap<String, Py<PyModule>>,
}

impl PythonInterpreterActor {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
        }
    }
}

impl Actor for PythonInterpreterActor {
    type Context = Context<Self>;
}

impl Handler<LoadComponents> for PythonInterpreterActor {
    type Result = ();

    fn handle(&mut self, msg: LoadComponents, _ctx: &mut Self::Context) -> Self::Result {
        Python::attach(|py| {
            for (name, component) in msg.components {
                let py_path = std::path::Path::new(&component.template_path).with_extension("py");
                if !py_path.exists() {
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
                let name_cstr = CString::new(name.as_str()).unwrap();

                match PyModule::from_code(py, &code_cstr, &path_cstr, &name_cstr) {
                    Ok(module) => {
                        self.modules.insert(name, module.into());
                    }
                    Err(e) => {
                        log::error!("Failed to load component {}: {}", name, e);
                    }
                }
            }
        });
    }
}

// Define the handler for the RenderComponent message
impl Handler<RenderComponent> for PythonInterpreterActor {
    type Result = Result<HashMap<String, Value>, Error>;

    fn handle(&mut self, msg: RenderComponent, _ctx: &mut Self::Context) -> Self::Result {
        Python::attach(|py| {
            let module = self
                .modules
                .get(&msg.name)
                .ok_or_else(|| Error::new(ErrorKind::NotFound, "Component not found"))?;

            let render_fn = module.getattr(py, "render")
                .map_err(|e| pyerr_to_io_error(e, py))?;

            let result = render_fn.call0(py)
                .map_err(|e| pyerr_to_io_error(e, py))?;

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