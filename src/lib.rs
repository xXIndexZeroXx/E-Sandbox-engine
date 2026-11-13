//! WebAssembly bindings for `ngspice-rs`.
//!
//! The editor writes a SPICE deck, this runs every analysis card in it, and the
//! results come back as plots. Small metadata crosses as JSON; waveforms cross
//! as `Float64Array`, one column at a time, because a long transient has far too
//! many numbers to be worth serializing.

use std::path::Path;

use ngspice_rs::analysis::batch;
use ngspice_rs::analysis::driver::runner;
use ngspice_rs::analysis::{Plot, PlotFlags, PointSink, RunConfig};
use ngspice_rs::netlist::Parser;
use ngspice_rs::netlist::source::parse_deck_text;
use ngspice_rs::primitives::{AnalysisKind, Complex, SpiceError, SpiceResult};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

fn to_js_error(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// The engine's version, for the UI's footer.
#[wasm_bindgen]
pub fn version() -> String {
    "ngspice-rs 0.4 (b9fc7bd)".to_owned()
}

#[derive(Serialize)]
struct PlotMeta {
    name: String,
    plotname: String,
    complex: bool,
    variables: Vec<String>,
    points: usize,
}

/// Every plot a deck produced, in ngspice batch order.
#[wasm_bindgen]
pub struct Results {
    names: Vec<String>,
    plots: Vec<Plot>,
}

#[wasm_bindgen]
impl Results {
    /// JSON array of `{name, plotname, complex, variables, points}`.
    pub fn meta(&self) -> String {
        let meta: Vec<PlotMeta> = self
            .plots
            .iter()
            .zip(&self.names)
            .map(|(plot, name)| PlotMeta {
                name: name.clone(),
                plotname: plot.plotname.clone(),
                complex: plot.flags == PlotFlags::Complex,
                variables: plot.variables.iter().map(|v| v.name.clone()).collect(),
                points: plot.point_count(),
            })
            .collect();
        serde_json::to_string(&meta).expect("plot metadata serializes")
    }

    /// Real part of one column of one plot.
    pub fn real(&self, plot: usize, variable: usize) -> Vec<f64> {
        self.column(plot, variable, |c| c.re)
    }

    /// Imaginary part of one column of one plot (zeros for real plots).
    pub fn imag(&self, plot: usize, variable: usize) -> Vec<f64> {
        self.column(plot, variable, |c| c.im)
    }
}

impl Results {
    fn column(&self, plot: usize, variable: usize, part: impl Fn(&Complex) -> f64) -> Vec<f64> {
        self.plots
            .get(plot)
            .map(|p| p.points.iter().map(|row| part(&row[variable])).collect())
            .unwrap_or_default()
    }
}

/// Hands each accepted timepoint to JS as it is accepted. JS answers `false`
/// to stop the run; otherwise it may block (the worker waits for the screen
/// to catch up), which is what turns a batch transient into a live one.
struct LiveSink<'a> {
    on_header: &'a js_sys::Function,
    on_point: &'a js_sys::Function,
    row: Vec<f64>,
}

impl PointSink for LiveSink<'_> {
    fn begin(&mut self, header: &Plot) -> SpiceResult<()> {
        let names: Vec<&str> = header.variables.iter().map(|v| v.name.as_str()).collect();
        let json = serde_json::to_string(&names).expect("names serialize");
        self.on_header
            .call1(&JsValue::NULL, &JsValue::from_str(&json))
            .map_err(|_| stopped())?;
        self.row = Vec::with_capacity(names.len());
        Ok(())
    }

    fn point(&mut self, values: &[Complex]) -> SpiceResult<()> {
        self.row.clear();
        self.row.extend(values.iter().map(|c| c.re));
        let array = js_sys::Float64Array::from(self.row.as_slice());
        match self.on_point.call1(&JsValue::NULL, &array) {
            Ok(v) if v.as_bool() == Some(false) => Err(stopped()),
            Ok(_) => Ok(()),
            Err(_) => Err(stopped()),
        }
    }
}

/// The error a run ends with when JS asked it to stop. JS knows it asked.
fn stopped() -> SpiceError {
    SpiceError::Numerical {
        context: "live run".to_owned(),
        message: "stopped".to_owned(),
    }
}

/// Runs the deck's `.tran` card, streaming every accepted point through
/// `on_point(Float64Array)` after `on_header(namesJson)`. The first column is time.
#[wasm_bindgen(js_name = streamTransient)]
pub fn stream_transient(deck: &str, on_header: &js_sys::Function, on_point: &js_sys::Function) -> Result<(), JsError> {
    let netlist = Parser::new()
        .parse_deck(&parse_deck_text(Path::new("circuit.cir"), deck))
        .map_err(to_js_error)?;
    let config = RunConfig::from_netlist(&netlist).map_err(to_js_error)?;
    let card = netlist
        .analyses
        .iter()
        .find(|card| config.request_for(card).is_ok_and(|r| r.kind == AnalysisKind::Transient))
        .ok_or_else(|| JsError::new("the deck has no .tran card"))?;
    let request = config.request_for(card).map_err(to_js_error)?;
    let driver = runner(request.kind).map_err(to_js_error)?;
    let mut circuit = config.circuit(&netlist).map_err(to_js_error)?;
    let mut sink = LiveSink { on_header, on_point, row: Vec::new() };
    driver
        .run_into(&mut circuit, &request, &config.context(), &mut sink)
        .map_err(to_js_error)?;
    Ok(())
}

/// Runs every analysis card in `deck` (full deck text, `.end` included).
/// `.include`/`.lib` are not available: there is no filesystem in the browser.
#[wasm_bindgen]
pub fn simulate(deck: &str) -> Result<Results, JsError> {
    let netlist = Parser::new()
        .parse_deck(&parse_deck_text(Path::new("circuit.cir"), deck))
        .map_err(to_js_error)?;
    if netlist.analyses.is_empty() {
        return Err(JsError::new("the deck requests no analysis"));
    }
    let config = RunConfig::from_netlist(&netlist).map_err(to_js_error)?;
    let schedule = batch::schedule_evaluated(&netlist.analyses, &config).map_err(to_js_error)?;

    let mut names = Vec::new();
    let mut plots = Vec::new();
    // Like ngspice, each analysis inherits the matrix ordering and state of the
    // one before it.
    let mut previous: Option<ngspice_rs::devices::Circuit> = None;
    for entry in &schedule {
        let request = config
            .request_for(&netlist.analyses[entry.card_index])
            .map_err(to_js_error)?;
        let driver = runner(request.kind).map_err(to_js_error)?;
        let mut circuit = config.circuit(&netlist).map_err(to_js_error)?;
        if let Some(previous) = &previous {
            circuit.inherit_from(previous);
        }
        let produced = driver
            .run_plots(&mut circuit, &request, &config.context())
            .map_err(to_js_error)?;
        // `run_plots` may prepend a bias plot that the schedule does not name.
        let scheduled: Vec<&str> = entry.plot_names().collect();
        for (i, plot) in produced.into_iter().enumerate() {
            names.push(scheduled.get(i).map_or_else(|| plot.name.clone(), |n| (*n).to_owned()));
            plots.push(plot);
        }
        circuit.note_completed(entry.kind);
        previous = Some(circuit);
    }
    Ok(Results { names, plots })
}
