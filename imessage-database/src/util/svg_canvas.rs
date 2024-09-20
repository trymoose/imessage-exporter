use std::collections::HashMap;
use std::fmt::{Display};

pub struct SVGCanvas {
    width: usize,
    height: usize,
    title: String,
    canvas: String,
    defs: HashMap<String, String>,
}

impl SVGCanvas {
    pub fn new(width: usize, height: usize) -> SVGCanvas {
        SVGCanvas{
            width,
            height,
            title: String::new(),
            canvas: String::new(),
            defs: HashMap::new(),
        }
    }

    pub fn square(size: usize) -> SVGCanvas {
        SVGCanvas::new(size, size)
    }

    pub fn width(&self) -> usize {
        self.width
    }
    pub fn height(&self) -> usize {
        self.height
    }

    pub fn fit_x(&self, x: usize, max: usize) -> usize {
        (((x as f64) / (max as f64)) * (self.width as f64)).floor() as usize
    }

    pub fn fit_y(&self, y: usize, max: usize) -> usize {
        let ratio = (y as f64) / (max as f64);
        (ratio * (self.height as f64)).floor() as usize
    }

    pub fn set_title(&mut self, title: String) {
        self.title = title.clone();
    }

    pub fn write_elem(&mut self, elem: &str, attrs: HashMap<&str, String>, inner: Option<String>) {
        self.canvas.push_str(Self::generate_elem(elem, attrs, inner).as_str());
    }

    pub fn generate_elem(elem: &str, attrs: HashMap<&str, String>, inner: Option<String>) -> String {
        let suffix = inner.map(|body| format!(">\n{body}\n</{elem}>")).unwrap_or_else(|| " />\n".to_string());
        let attr_str = attrs.iter().map(|entry| {
            let (name, value) = entry;
            format!(r#" {name}="{value}""#)
        }).collect::<Vec<String>>().join("");
        format!("<{elem}{attr_str}{suffix}")
    }

    pub fn add_def(&mut self, id: &str, elem: &str, attrs: HashMap<&str, String>, inner: Option<String>) {
        let mut id_attrs = HashMap::new();
        attrs.clone_into(&mut id_attrs);
        id_attrs.insert("id", id.to_string());
        self.defs.insert(id.to_string(), Self::generate_elem(elem, id_attrs, inner));
    }
}

impl Display for SVGCanvas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let width = self.width;
        let height = self.height;
        let canvas = &self.canvas;
        let title = &self.title;
        let defs = self.defs.values().map(|e| e.clone()).collect::<Vec<String>>().join("\n");

        write!(f, r#"<svg viewBox="0 0 {width} {height}" preserveAspectRatio="xMidYMid meet" width="100%" height="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
<title>{title}</title>
<defs>
{defs}
</defs>
{canvas}
</svg>"#)
    }
}