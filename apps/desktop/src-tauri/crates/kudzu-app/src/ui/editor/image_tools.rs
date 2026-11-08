//! `image-tools.tsx`: embed a picture in a formatted snippet. The image is resized to at most
//! 512 px and re-encoded as a PNG data URL of at most 192 KB, shown at a display width (16–512
//! px, 240 at most by default) with a text alternative, and saved as an image span over
//! `[alt]` in the source.

use std::path::PathBuf;

use base64::Engine;
use kudzu_core::client::editor_formatting::{self as ef, TextEdit as Edit};
use kudzu_core::client::js;
use kudzu_core::formatting::{FormatSpan, FormatStyle};
use rune::{col, el, text, Align, Cx, El, FileDialogOptions, Pixels, TextEdit};

use super::template::now;
use super::{ensure, template_changed, EditorState};
use crate::ui::icons;
use crate::ui::kit::button::{Button, Size, Variant};
use crate::ui::kit::input::Input;
use crate::ui::kit::popover::Popover;
use crate::ui::main::MainUi;
use crate::ui::theme::{radius, text as ty, weight, Theme, WeightOf};
use crate::ui::theme_for;

const MAX_DATA_URL: usize = 196_608;
const MAX_TOTAL: usize = 524_288;

#[derive(Clone)]
pub struct Prepared {
    pub data_url: String,
    pub width: u32,
    pub pixels: Pixels,
    pub size: (u32, u32),
}

#[derive(Default, Clone)]
pub struct ImageState {
    pub open: bool,
    pub image: Option<Prepared>,
    pub file: Option<String>,
    pub label: TextEdit,
    pub width: TextEdit,
    pub error: String,
    pub loading: bool,
    request: u64,
}

/// `compactImage`: checks, downsizes and re-encodes a picked file.
pub fn compact_image(path: &std::path::Path) -> Result<Prepared, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp") {
        return Err("Choose a PNG, JPEG, or WebP image.".into());
    }
    let meta = std::fs::metadata(path).map_err(|_| "This image couldn't be read.".to_string())?;
    if meta.len() > 20 * 1024 * 1024 {
        return Err("Choose an image smaller than 20 MB.".into());
    }
    let bytes = std::fs::read(path).map_err(|_| "This image couldn't be read.".to_string())?;
    let img = image::load_from_memory(&bytes).map_err(|_| "This image couldn't be opened.".to_string())?;
    let (w, h) = (img.width(), img.height());
    let mut scale = (512.0 / w.max(h) as f64).min(1.0);
    for _ in 0..8 {
        let cw = ((w as f64 * scale).round() as u32).max(1);
        let ch = ((h as f64 * scale).round() as u32).max(1);
        let resized = img.resize_exact(cw, ch, image::imageops::FilterType::Triangle).to_rgba8();
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(resized.clone())
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|_| "This image couldn't be opened.".to_string())?;
        let data_url = format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&png));
        if data_url.len() <= MAX_DATA_URL {
            let pixels = Pixels::new();
            pixels.set(cw, ch, resized.into_raw());
            return Ok(Prepared { data_url, width: cw.clamp(16, 240), pixels, size: (cw, ch) });
        }
        scale *= 0.75;
    }
    Err("This image is too detailed to embed. Try a smaller image.".into())
}

fn reset(st: &mut ImageState, open: bool) {
    st.request += 1;
    st.open = open;
    st.image = None;
    st.file = None;
    st.error.clear();
    st.loading = false;
}

fn choose(_s: &mut MainUi, tx: &mut rune::Tx) {
    let options = FileDialogOptions { title: "Choose image".into(), ..Default::default() }.filter("Images", &["png", "jpg", "jpeg", "webp"]);
    tx.open_file(options, |s: &mut MainUi, result, _| {
        let Ok(Some(path)) = result else { return };
        picked(s, path);
    });
}

fn picked(s: &mut MainUi, path: PathBuf) {
    ensure(s);
    let st = &mut s.editor.image;
    st.request += 1;
    let token = st.request;
    st.loading = true;
    st.error.clear();
    st.image = None;
    st.file = path.file_name().map(|n| n.to_string_lossy().into_owned());
    let stem = path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    s.spawn(move || compact_image(&path), move |s, result, _| {
        let st = &mut s.editor.image;
        if st.request != token {
            return;
        }
        st.loading = false;
        match result {
            Ok(p) => {
                st.width = TextEdit::new(p.width.to_string());
                st.label = TextEdit::new(stem.clone());
                st.image = Some(p);
            }
            Err(e) => st.error = e,
        }
    });
}

/// Inserts the prepared image over `[alt]` at the selection, as one undo step.
fn submit(s: &mut MainUi) {
    ensure(s);
    let t = &s.editor.template;
    let st = &s.editor.image;
    let Some(img) = st.image.clone() else { return };
    if st.loading || t.composing() {
        return;
    }
    let alt: String = js::trim(&st.label.text).chars().map(|c| if matches!(c, '{' | '}' | '\r' | '\n') { ' ' } else { c }).collect();
    let text = format!("[{}]", if alt.is_empty() { "Image" } else { &alt });
    let doc = &t.doc;
    let (from, to) = doc.selected_range();
    let edits = [Edit { from, to, insert: text.clone() }];
    let Some(desc) = ef::edits_desc(doc.text(), &edits) else { return };
    let next = ef::apply_edits(doc.text(), &edits);
    let mapped = ef::update_formatting(doc.spans(), Some(&desc), &[], &next).unwrap_or_default();
    let end = from + js::len16(&text);
    let mut spans: Vec<FormatSpan> = mapped.into_iter().filter(|sp| !(matches!(sp.style, FormatStyle::Image { .. }) && sp.start < end && sp.end > from)).collect();
    let size = spans.iter().map(|sp| if let FormatStyle::Image { data_url, .. } = &sp.style { data_url.len() } else { 0 }).sum::<usize>() + img.data_url.len();
    if size > MAX_TOTAL {
        s.editor.image.error = "This snippet has too many images. Remove an image or use a smaller one.".into();
        return;
    }
    let width = st.width.text.trim().parse::<u32>().unwrap_or(img.width).clamp(16, 512);
    spans.push(FormatSpan { start: from, end, style: FormatStyle::Image { data_url: img.data_url.clone(), width } });
    s.editor.template.doc.replace_with_spans(from, to, &text, spans, now());
    reset(&mut s.editor.image, false);
    s.editor.template.focus();
    template_changed(s);
}

pub fn view(_s: &MainUi, st: &EditorState, cx: &mut Cx<MainUi>, layer: &mut Vec<El>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let im = &st.image;
    let trigger = Button::new("Image").variant(Variant::Ghost).size(Size::Sm).icon(&icons::IMAGE_PLUS).color(t.muted_foreground).id("format-image").build(th).label("Insert image");
    let mut pop = Popover::new("format-image-pop")
        .open(im.open)
        .on_open_change(|s: &mut MainUi, open, _| {
            ensure(s);
            reset(&mut s.editor.image, open);
            if !open {
                s.editor.template.focus();
            }
        })
        .trigger(trigger)
        .align(Align::Start)
        .width(320.0)
        .label("Embed an image");
    if im.open {
        // `<Input type="file">`: a Choose File button and the picked file's name.
        let file = rune::row()
            .key("image-file")
            .w_full()
            .h(32.0)
            .px(4.0)
            .gap(8.0)
            .center_y()
            .radius(radius::LG)
            .fill(if th.dark { rune::Color::BLACK.alpha(0.15) } else { Theme::tint(t.foreground, 0.025) })
            .role("button")
            .label("Choose image")
            .focusable(true)
            .on_press_tx(choose)
            .child(el().h(24.0).px(8.0).center().radius(radius::MD).fill(t.secondary).child(text("Choose File").font(ty::SM.weight_of(weight::MEDIUM)).color(t.foreground)))
            .child(text(im.file.clone().unwrap_or_else(|| "No file chosen".into())).font(ty::SM).color(t.muted_foreground).truncate().min_w(0.0).shrink());
        let mut form = col()
            .gap(12.0)
            .child(text("Embed an image").font(ty::SM.weight_of(weight::MEDIUM)).color(t.foreground))
            .child(text("Saved with your snippet. Large images are resized automatically.").font(ty::XS).color(t.muted_foreground).wrap())
            .child(file);
        if let Some(img) = &im.image {
            let width = im.width.text.trim().parse::<f32>().unwrap_or(img.width as f32);
            let ratio = img.size.1 as f32 / img.size.0.max(1) as f32;
            let (mut w, mut h) = (width.min(300.0), width.min(300.0) * ratio);
            if h > 160.0 {
                w *= 160.0 / h;
                h = 160.0;
            }
            form = form
                .child(el().key("image-preview").w_full().items_center().child(el().size(w, h).pixels(&img.pixels).label(im.label.text.clone()).role("img")))
                .child(
                    col().key("image-alt").gap(4.0).child(text("Text alternative").font(ty::XS).color(t.foreground)).child(
                        Input::new(&im.label)
                            .label("Text alternative")
                            .on_edit(|s: &mut MainUi| {
                                ensure(s);
                                &mut s.editor.image.label
                            })
                            .on_change(|s: &mut MainUi, _| {
                                let l = &mut s.editor.image.label;
                                if l.text.chars().count() > 200 {
                                    let cut: String = l.text.chars().take(200).collect();
                                    l.replace_text(cut);
                                }
                            })
                            .on_submit(|s: &mut MainUi, _| submit(s))
                            .build(th),
                    ),
                )
                .child(
                    col().key("image-width").gap(4.0).child(text("Width in pixels").font(ty::XS).color(t.foreground)).child(
                        Input::new(&im.width)
                            .label("Width in pixels")
                            .on_edit(|s: &mut MainUi| {
                                ensure(s);
                                &mut s.editor.image.width
                            })
                            .on_change(|s: &mut MainUi, _| {
                                let w = &mut s.editor.image.width;
                                let n = w.text.trim().parse::<f64>().ok().filter(|n| *n != 0.0).unwrap_or(16.0);
                                let v = (n.clamp(16.0, 512.0) as u32).to_string();
                                if v != w.text {
                                    w.replace_text(v);
                                }
                            })
                            .on_submit(|s: &mut MainUi, _| submit(s))
                            .build(th),
                    ),
                );
        }
        if !im.error.is_empty() {
            form = form.child(text(im.error.clone()).key("image-error").font(ty::XS).color(t.destructive).wrap().role("alert"));
        }
        form = form.child(
            Button::new(if im.loading { "Preparing image…" } else { "Insert image" })
                .size(Size::Sm)
                .full_width()
                .disabled(im.image.is_none() || im.loading)
                .id("image-insert")
                .on_press(submit)
                .build(th).key("image-submit"),
        );
        pop = pop.child(form);
    }
    pop.build(cx, layer)
}
