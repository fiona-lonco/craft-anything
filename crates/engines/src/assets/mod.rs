//! Generated game assets: pictures (textures, skies, concept art) and game-ready 3D models, made
//! by a paid provider. Each provider is an [`AssetProvider`]; an [`AssetProviders`] registry holds
//! them (built in: [`fal::Fal`]). The models and their prices come from the `assets.toml` table
//! ([`Catalogue`]), so a cost is known before anything is spent, and every generation reports
//! what it cost. Audio would be one more [`AssetRequest`] variant and table.

pub mod custom;
pub mod fal;

use craft_anything_evidence::{ErrorCode, ToolError};
use serde::Deserialize;
use std::ops::RangeInclusive;
use std::sync::{Arc, OnceLock};

/// What kind of asset a request makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AssetKind {
    Image,
    Model3d,
    Video,
}

/// The id Settings uses for the person's own picture model ([`CustomModel`]).
pub const CUSTOM_IMAGE: &str = "custom";

/// How a model of the person's own is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomKind {
    /// A model at fal.ai, by its address there (`fal-ai/flux-pro/v1.1`), with the fal.ai key.
    #[default]
    Fal,
    /// Any web API, here or online: a URL, a JSON body with placeholders and headers (its key).
    Http,
    /// A program on this computer that writes the picture or video to `{output}`.
    Command,
}

/// A model of the person's own choosing, for pictures or videos: at fal.ai, behind any web API
/// (OpenAI, Replicate, a local Stable Diffusion or ComfyUI…), or a program on this computer. With
/// what one result costs, so the budget can count it before it's made.
///
/// Placeholders in `body`, `args` and `endpoint`: `{prompt}`, `{width}`, `{height}`, `{size}`
/// (`1024x1024`), `{aspect_ratio}` (`16:9`), `{image_url}` (a picture to start from, or empty),
/// `{kind}` (`image` or `video`) and, for a program, `{output}` (the file it must write).
#[derive(Debug, Clone, PartialEq, serde::Serialize, Deserialize, Default)]
pub struct CustomModel {
    #[serde(default)]
    pub kind: CustomKind,
    /// fal.ai: the model's address there. Web API: its URL. Program: the program to run.
    pub endpoint: String,
    /// What one picture or video costs (US dollars); 0 for one that's free (a local model).
    #[serde(default)]
    pub usd_each: f64,
    /// Web API: the JSON sent, with placeholders. Empty: `{"prompt": …, "width": …, "height": …}`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub body: String,
    /// Web API: headers sent with it (its key: `Authorization: Bearer …`).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub headers: std::collections::BTreeMap<String, String>,
    /// Web API: where the answer has the picture or video, as a JSON pointer (`/data/0/b64_json`):
    /// a web address, a `data:` URI or base64. Empty: the usual places are tried.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub result: String,
    /// Program: its arguments, with placeholders.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Seconds one result may take (default: ten minutes for a picture, thirty for a video).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
    /// Web API or program: the key that takes the place of `{key}` (never shown by settings_show).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key: String,
}

impl CustomModel {
    /// The address or program, tidied: for fal.ai, a pasted `https://fal.ai/models/` or
    /// `https://fal.run/` is taken off.
    pub fn endpoint(&self) -> String {
        match self.kind {
            CustomKind::Fal => {
                let e = self.endpoint.trim().trim_matches('/');
                let e = ["https://fal.ai/models/", "https://fal.run/", "http://fal.ai/models/", "fal.ai/models/"].iter().fold(e, |e, p| e.strip_prefix(p).unwrap_or(e));
                e.trim_matches('/').to_string()
            }
            CustomKind::Http | CustomKind::Command => self.endpoint.trim().to_string(),
        }
    }

    /// The provider id that makes it ([`fal::ID`], [`custom::HTTP`] or [`custom::COMMAND`]).
    pub fn provider(&self) -> &'static str {
        match self.kind {
            CustomKind::Fal => fal::ID,
            CustomKind::Http => custom::HTTP,
            CustomKind::Command => custom::COMMAND,
        }
    }

    /// What's wrong with it, if anything.
    pub fn problem(&self) -> Option<String> {
        let e = self.endpoint();
        if !price_ok(self.usd_each) {
            return Some("The price must be zero or more.".into());
        }
        match self.kind {
            CustomKind::Fal => {
                if e.is_empty() {
                    return Some("Give the model's address at fal.ai, like fal-ai/flux-pro/v1.1.".into());
                }
                if !e.contains('/') || e.contains(char::is_whitespace) || e.contains("..") || e.contains("://") {
                    return Some(format!("\"{e}\" isn't a fal.ai model address; it looks like fal-ai/flux-pro/v1.1."));
                }
            }
            CustomKind::Http => {
                if !(e.starts_with("https://") || e.starts_with("http://")) {
                    return Some("Give the API's address, starting with https:// (or http:// for one on your computer or network).".into());
                }
                if !self.body.trim().is_empty() && custom::fill_json(&self.body, &custom::Fill::sample()).is_err() {
                    return Some("The body isn't valid JSON once {prompt} and the others are filled in.".into());
                }
                if !self.result.trim().is_empty() && !self.result.trim().starts_with('/') {
                    return Some("Where the answer has the result must start with /, like /data/0/b64_json.".into());
                }
                if self.key.trim().is_empty() && (e.contains(custom::KEY) || self.headers.values().any(|v| v.contains(custom::KEY))) {
                    return Some("Add your key for this service.".into());
                }
            }
            CustomKind::Command => {
                if e.is_empty() {
                    return Some("Give the program to run.".into());
                }
                if !self.args.iter().any(|a| a.contains("{output}")) {
                    return Some("Put {output} in the arguments: it's the file the program must write.".into());
                }
            }
        }
        None
    }

    /// As a picture model.
    pub fn as_image(&self) -> ImageModel {
        let endpoint = self.endpoint();
        ImageModel {
            id: CUSTOM_IMAGE.into(),
            provider: self.provider().into(),
            name: endpoint.clone(),
            endpoint,
            usd_per_megapixel: 0.0,
            usd_per_image: self.usd_each.max(0.0),
            // fal: told the shape both ways, since its page says which it takes.
            size_by: SizeBy::Both,
            licence: "See the model's own terms".into(),
            custom: Some(self.clone()),
        }
    }

    /// As a video model.
    pub fn as_video(&self) -> VideoModel {
        let endpoint = self.endpoint();
        VideoModel { name: endpoint.clone(), provider: self.provider().into(), endpoint, usd_each: self.usd_each.max(0.0), custom: Some(self.clone()) }
    }
}

/// A video model: where it runs and what one video costs.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoModel {
    pub provider: String,
    pub name: String,
    pub endpoint: String,
    pub usd_each: f64,
    /// How the person set it up (a web API's body and headers, a program's arguments).
    pub custom: Option<CustomModel>,
}

/// Picture shapes, with their pixel size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// 1024×1024: objects, textures, icons.
    Square,
    /// 1024×576: skies, backgrounds, title screens.
    Wide,
    /// 576×1024: posters, tall props.
    Tall,
}

impl Shape {
    pub fn parse(s: &str) -> Option<Shape> {
        match s {
            "square" => Some(Shape::Square),
            "wide" => Some(Shape::Wide),
            "tall" => Some(Shape::Tall),
            _ => None,
        }
    }

    pub fn megapixels(self) -> f64 {
        match self {
            Shape::Square => 1024.0 * 1024.0 / 1e6,
            Shape::Wide | Shape::Tall => 1024.0 * 576.0 / 1e6,
        }
    }
}

/// How a picture model is told the shape it should make.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizeBy {
    /// A named size (`square_hd`, `landscape_16_9`), as FLUX takes it.
    #[default]
    ImageSize,
    /// An aspect ratio (`1:1`, `16:9`), as Nano Banana Pro takes it.
    AspectRatio,
    /// Both (a model of the person's own, whose page says which it takes).
    Both,
}

/// A picture model from the table.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ImageModel {
    /// The table id (`flux-dev`), used in Settings.
    pub id: String,
    /// The provider that runs it (`fal`).
    pub provider: String,
    /// Shown to the person.
    pub name: String,
    /// The model's address at the provider.
    pub endpoint: String,
    /// The price by size; a model billed per picture leaves it at 0.
    #[serde(default)]
    pub usd_per_megapixel: f64,
    /// The price per picture, whatever its size.
    #[serde(default)]
    pub usd_per_image: f64,
    #[serde(default)]
    pub size_by: SizeBy,
    /// The model weights' licence, for the person choosing one.
    #[serde(default)]
    pub licence: String,
    /// A model of the person's own: how it's reached (not in the table).
    #[serde(skip)]
    pub custom: Option<CustomModel>,
}

impl ImageModel {
    /// What a picture of this shape costs.
    pub fn cost(&self, shape: Shape) -> f64 {
        self.usd_per_image + self.usd_per_megapixel * shape.megapixels()
    }
}

/// A 3D model generator from the table (a picture in, a GLB out).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Model3d {
    pub id: String,
    pub provider: String,
    pub name: String,
    pub endpoint: String,
    /// Per model, untextured and textured.
    pub usd: f64,
    pub textured_usd: f64,
    /// The face counts it accepts, `[fewest, most]`.
    pub faces: [u32; 2],
    #[serde(default)]
    pub licence: String,
}

impl Model3d {
    pub fn cost(&self, textured: bool) -> f64 {
        if textured { self.textured_usd } else { self.usd }
    }

    pub fn faces(&self) -> RangeInclusive<u32> {
        let [low, high] = self.faces;
        low..=high
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct Defaults {
    image: String,
    model_3d: String,
}

/// The models assets are made with and their prices (`assets.toml`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Catalogue {
    default: Defaults,
    #[serde(default)]
    image: Vec<ImageModel>,
    #[serde(default)]
    model_3d: Vec<Model3d>,
}

fn price_ok(usd: f64) -> bool {
    usd.is_finite() && usd >= 0.0
}

impl Catalogue {
    /// The built-in table. One that doesn't parse (a broken edit) gives [`Catalogue::fallback`],
    /// so assets keep working at the known prices; a test keeps the table parsing.
    pub fn built_in() -> &'static Catalogue {
        static TABLE: OnceLock<Catalogue> = OnceLock::new();
        TABLE.get_or_init(|| {
            Catalogue::parse(include_str!("assets.toml")).unwrap_or_else(|e| {
                log::warn!("assets.toml is unreadable ({e}); using the built-in asset models");
                Catalogue::fallback()
            })
        })
    }

    /// A table from TOML text. Entries with a price that isn't a positive number, an empty
    /// endpoint or an empty face range are left out (asking for them gives a clear error).
    pub fn parse(text: &str) -> Result<Catalogue, toml::de::Error> {
        let mut c: Catalogue = toml::from_str(text)?;
        c.image.retain(|m| price_ok(m.usd_per_megapixel) && price_ok(m.usd_per_image) && !m.endpoint.trim().is_empty());
        c.model_3d.retain(|m| {
            price_ok(m.usd) && price_ok(m.textured_usd) && !m.endpoint.trim().is_empty() && {
                let [low, high] = m.faces;
                low <= high
            }
        });
        Ok(c)
    }

    /// The defaults the table ships with, in code: Nano Banana Pro and Tripo P1 on fal.ai.
    pub fn fallback() -> Catalogue {
        Catalogue {
            default: Defaults { image: "nano-banana-pro".into(), model_3d: "tripo-p1".into() },
            image: vec![ImageModel {
                id: "nano-banana-pro".into(),
                provider: fal::ID.into(),
                name: "Nano Banana Pro".into(),
                endpoint: "fal-ai/nano-banana-pro".into(),
                usd_per_megapixel: 0.0,
                usd_per_image: 0.15,
                size_by: SizeBy::AspectRatio,
                licence: "Commercial use enabled through fal.ai".into(),
                custom: None,
            }],
            model_3d: vec![Model3d {
                id: "tripo-p1".into(),
                provider: fal::ID.into(),
                name: "Tripo P1".into(),
                endpoint: "tripo3d/p1/image-to-3d".into(),
                usd: 0.40,
                textured_usd: 0.50,
                faces: [48, 20_000],
                licence: String::new(),
            }],
        }
    }

    pub fn images(&self) -> &[ImageModel] {
        &self.image
    }

    pub fn models_3d(&self) -> &[Model3d] {
        &self.model_3d
    }

    /// The picture model `id` names, or the default one when `id` is `None` or empty.
    pub fn image_model(&self, id: Option<&str>) -> Result<&ImageModel, ToolError> {
        let id = id.map(str::trim).filter(|s| !s.is_empty()).unwrap_or(&self.default.image);
        self.image.iter().find(|m| m.id == id).ok_or_else(|| {
            let known: Vec<&str> = self.image.iter().map(|m| m.id.as_str()).collect();
            missing("picture model", id, &known)
        })
    }

    /// The picture model Settings chose: [`CUSTOM_IMAGE`] is the person's own (`custom`), any
    /// other id is looked up as [`Catalogue::image_model`] does.
    pub fn picture_model(&self, id: Option<&str>, custom: Option<&CustomModel>) -> Result<std::borrow::Cow<'_, ImageModel>, ToolError> {
        if id.map(str::trim) == Some(CUSTOM_IMAGE) {
            let c = custom.ok_or_else(|| {
                ToolError::new(ErrorCode::BadSettings, "Your own picture model is chosen, but no model address is set.")
                    .with_remediation("Set it in Settings → Builds → Picture model, or choose another one.")
            })?;
            if let Some(problem) = c.problem() {
                return Err(ToolError::new(ErrorCode::BadSettings, problem).with_remediation("Fix it in Settings → Builds → Picture model."));
            }
            return Ok(std::borrow::Cow::Owned(c.as_image()));
        }
        self.image_model(id).map(std::borrow::Cow::Borrowed)
    }

    /// The 3D model generator `id` names, or the default one.
    pub fn model_3d(&self, id: Option<&str>) -> Result<&Model3d, ToolError> {
        let id = id.map(str::trim).filter(|s| !s.is_empty()).unwrap_or(&self.default.model_3d);
        self.model_3d.iter().find(|m| m.id == id).ok_or_else(|| {
            let known: Vec<&str> = self.model_3d.iter().map(|m| m.id.as_str()).collect();
            missing("3D model generator", id, &known)
        })
    }
}

fn missing(what: &str, id: &str, known: &[&str]) -> ToolError {
    let remediation = if known.is_empty() {
        "The asset table lists none; generate assets with simple shapes and colours instead.".to_string()
    } else {
        format!("Choose one of: {} (Settings → AI → Game assets, or [assets] in config.toml).", known.join(", "))
    };
    ToolError::new(ErrorCode::BadSettings, format!("There is no {what} called \"{id}\" in the asset table.")).with_remediation(remediation)
}

/// One thing to generate, with the table entry that says where and at what price.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum AssetRequest<'a> {
    /// A picture (PNG) from a description.
    Image { model: &'a ImageModel, prompt: &'a str, shape: Shape },
    /// A 3D model (GLB) of at most `faces` faces from a picture: a web address, or a `data:` URI.
    Model3d { model: &'a Model3d, image_url: &'a str, textured: bool, faces: u32 },
    /// A video (MP4) from a description, or from a picture (a web address or a `data:` URI).
    Video { model: &'a VideoModel, prompt: &'a str, shape: Shape, image_url: Option<&'a str> },
}

impl AssetRequest<'_> {
    pub fn kind(&self) -> AssetKind {
        match self {
            AssetRequest::Image { .. } => AssetKind::Image,
            AssetRequest::Model3d { .. } => AssetKind::Model3d,
            AssetRequest::Video { .. } => AssetKind::Video,
        }
    }

    /// The provider the table entry names.
    pub fn provider(&self) -> &str {
        match self {
            AssetRequest::Image { model, .. } => &model.provider,
            AssetRequest::Model3d { model, .. } => &model.provider,
            AssetRequest::Video { model, .. } => &model.provider,
        }
    }

    /// The table's price for it.
    pub fn listed_cost(&self) -> f64 {
        match self {
            AssetRequest::Image { model, shape, .. } => model.cost(*shape),
            AssetRequest::Model3d { model, textured, .. } => model.cost(*textured),
            AssetRequest::Video { model, .. } => model.usd_each,
        }
    }
}

/// A generated file: where the provider keeps it, its bytes, and what it cost.
#[derive(Debug, Clone)]
pub struct Generated {
    pub url: String,
    pub bytes: Vec<u8>,
    pub cost_usd: f64,
}

/// A paid service that generates assets.
pub trait AssetProvider: Send + Sync {
    /// Stable id, named by the table's `provider` (`"fal"`).
    fn id(&self) -> &'static str;
    /// Shown to the person ("fal.ai").
    fn name(&self) -> &'static str;
    /// The environment variable the provider's own tools read its key from (`FAL_KEY`), used
    /// when no key is stored.
    fn key_env(&self) -> Option<&'static str> {
        None
    }
    /// Whether it needs a key of its own from Settings (a web API of the person's carries its own
    /// in its headers, and a program needs none).
    fn needs_key(&self) -> bool {
        true
    }
    /// Where the person gets a key (shown as a link in Settings → AI).
    fn keys_page(&self) -> Option<&'static str> {
        None
    }
    /// What `req` will cost, before anything is spent (the budget is checked against it).
    fn estimate(&self, req: &AssetRequest) -> Result<f64, ToolError> {
        Ok(req.listed_cost())
    }
    /// Make it with the person's `key`; the result carries what it actually cost.
    fn generate(&self, key: &str, req: &AssetRequest) -> Result<Generated, ToolError>;
}

/// The asset providers a project can use.
#[derive(Clone)]
pub struct AssetProviders {
    providers: Vec<Arc<dyn AssetProvider>>,
}

impl std::fmt::Debug for AssetProviders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.providers.iter().map(|p| p.id())).finish()
    }
}

impl Default for AssetProviders {
    fn default() -> Self {
        AssetProviders::built_in()
    }
}

impl AssetProviders {
    /// fal.ai.
    pub fn built_in() -> AssetProviders {
        AssetProviders { providers: vec![Arc::new(fal::Fal::default()), Arc::new(custom::Http), Arc::new(custom::Command)] }
    }

    /// Add a provider, or replace the one with the same id.
    pub fn register(&mut self, provider: Arc<dyn AssetProvider>) {
        self.providers.retain(|p| p.id() != provider.id());
        self.providers.push(provider);
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn AssetProvider>> {
        self.providers.iter().find(|p| p.id() == id).cloned()
    }

    pub fn ids(&self) -> Vec<&'static str> {
        self.providers.iter().map(|p| p.id()).collect()
    }

    /// The provider that makes `req`.
    pub fn for_request(&self, req: &AssetRequest) -> Result<Arc<dyn AssetProvider>, ToolError> {
        let id = req.provider();
        self.get(id).ok_or_else(|| {
            ToolError::new(ErrorCode::FeatureMissing, format!("No asset provider called \"{id}\" is available."))
                .with_remediation("Check the asset table, or generate assets with simple shapes and colours instead.")
        })
    }
}

/// A price in prompt text: "$0.53", or "under $0.01" for a fraction of a cent.
pub fn usd(x: f64) -> String {
    if x > 0.0 && x < 0.005 { "under $0.01".into() } else { format!("${x:.2}") }
}

/// "about $0.53", or "under $0.01".
pub fn about_usd(x: f64) -> String {
    if x > 0.0 && x < 0.005 { usd(x) } else { format!("about {}", usd(x)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_parses_and_its_defaults_match_the_code() {
        let t = Catalogue::parse(include_str!("assets.toml")).unwrap();
        let f = Catalogue::fallback();
        assert_eq!(t.image_model(None).unwrap(), f.image_model(None).unwrap());
        assert_eq!(t.model_3d(None).unwrap(), f.model_3d(None).unwrap());
        assert_eq!(Catalogue::built_in(), &t);
        let schnell = t.image_model(Some("flux-schnell")).unwrap();
        assert_eq!(schnell.endpoint, "fal-ai/flux/schnell");
        assert!(schnell.cost(Shape::Square) < t.image_model(Some("flux-dev")).unwrap().cost(Shape::Square));
        let nano = t.image_model(None).unwrap();
        assert_eq!((nano.id.as_str(), nano.endpoint.as_str(), nano.size_by), ("nano-banana-pro", "fal-ai/nano-banana-pro", SizeBy::AspectRatio));
        assert_eq!(t.image_model(Some("flux-dev")).unwrap().size_by, SizeBy::ImageSize, "a named size unless the table says otherwise");
        for m in t.images() {
            assert!(AssetProviders::built_in().get(&m.provider).is_some(), "{} names an unknown provider", m.id);
        }
    }

    #[test]
    fn prices_and_face_limits() {
        let c = Catalogue::built_in();
        let flux = c.image_model(Some("flux-dev")).unwrap();
        assert!((flux.cost(Shape::Square) - 0.0262144).abs() < 1e-9, "1024×1024 at $0.025 per megapixel");
        assert!(flux.cost(Shape::Wide) < flux.cost(Shape::Square));
        let nano = c.image_model(None).unwrap();
        assert_eq!((nano.cost(Shape::Square), nano.cost(Shape::Wide)), (0.15, 0.15), "per picture, whatever its shape");
        let tripo = c.model_3d(Some(" ")).unwrap();
        assert_eq!((tripo.cost(true), tripo.cost(false)), (0.50, 0.40));
        assert_eq!(tripo.faces(), 48..=20_000);
        assert_eq!(about_usd(tripo.cost(true) + flux.cost(Shape::Square)), "about $0.53");
        assert_eq!(about_usd(tripo.cost(true) + nano.cost(Shape::Square)), "about $0.65");
        assert_eq!(about_usd(0.0031), "under $0.01");
        assert_eq!(usd(0.0262144), "$0.03");
    }

    #[test]
    fn a_broken_or_odd_table_never_panics() {
        assert!(Catalogue::parse("not = [toml").is_err());
        assert!(Catalogue::parse("").is_err(), "the defaults are required");
        let odd = Catalogue::parse(
            r#"
            [default]
            image = "free"
            model_3d = "upside-down"
            [[image]]
            id = "free"
            provider = "fal"
            name = "x"
            endpoint = "x"
            usd_per_image = -1.0
            [[model_3d]]
            id = "upside-down"
            provider = "fal"
            name = "x"
            endpoint = "x"
            usd = 0.1
            textured_usd = 0.2
            faces = [500, 10]
            "#,
        )
        .unwrap();
        let e = odd.image_model(None).unwrap_err();
        assert_eq!(e.code, ErrorCode::BadSettings);
        assert!(e.message.contains("\"free\""), "{}", e.message);
        assert!(odd.model_3d(None).is_err());
        let e = Catalogue::fallback().image_model(Some("dall-e")).unwrap_err();
        assert!(e.remediation.unwrap_or_default().contains("nano-banana-pro"));
    }

    #[test]
    fn the_registry_finds_the_provider_a_model_names() {
        struct Stub;
        impl AssetProvider for Stub {
            fn id(&self) -> &'static str {
                "stub"
            }
            fn name(&self) -> &'static str {
                "Stub"
            }
            fn generate(&self, _key: &str, req: &AssetRequest) -> Result<Generated, ToolError> {
                Ok(Generated { url: String::new(), bytes: Vec::new(), cost_usd: req.listed_cost() })
            }
        }
        let mut r = AssetProviders::built_in();
        assert_eq!(r.ids(), vec!["fal", "http", "command"]);
        let mut m = Catalogue::fallback().image_model(None).unwrap().clone();
        m.provider = "stub".into();
        let req = AssetRequest::Image { model: &m, prompt: "a crate", shape: Shape::Square };
        assert_eq!(r.for_request(&req).err().map(|e| e.code), Some(ErrorCode::FeatureMissing));
        r.register(Arc::new(Stub));
        let p = r.for_request(&req).unwrap();
        assert_eq!(p.estimate(&req).unwrap(), m.cost(Shape::Square));
        assert_eq!(p.generate("k", &req).unwrap().cost_usd, m.cost(Shape::Square));
        assert_eq!(req.kind(), AssetKind::Image);
    }
}
