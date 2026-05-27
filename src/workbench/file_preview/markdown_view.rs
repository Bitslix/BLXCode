//! Markdown preview renderer with inline Mermaid block support.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, read_workspace_text_file, PolicyKind};
use crate::workbench::file_preview::mermaid_glue::run_mermaid_on;
use crate::workbench::file_preview::util::{
    render_load_error, sanitize_markdown_html, FilePreviewError,
};
use crate::workbench::WorkbenchService;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use pulldown_cmark::{html as md_html, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement};

/// Renders markdown to sanitized HTML; fenced ```mermaid blocks become
/// `<pre class="mermaid …">…raw graph text…</pre>` sentinels picked up by the
/// post-mount effect.
fn render_markdown(src: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_SMART_PUNCTUATION);

    let parser = Parser::new_ext(src, opts);
    let mut buffered = Vec::new();
    let mut in_mermaid = false;
    let mut mermaid_buf = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang)))
                if lang.as_ref().eq_ignore_ascii_case("mermaid") =>
            {
                in_mermaid = true;
                mermaid_buf.clear();
            }
            Event::End(TagEnd::CodeBlock) if in_mermaid => {
                let escaped = html_escape(&mermaid_buf);
                buffered.push(Event::Html(
                    format!(r#"<pre class="mermaid file-preview__mermaid-inline">{escaped}</pre>"#)
                        .into(),
                ));
                in_mermaid = false;
                mermaid_buf.clear();
            }
            Event::Text(t) if in_mermaid => {
                mermaid_buf.push_str(&t);
            }
            other if !in_mermaid => buffered.push(other),
            _ => {}
        }
    }
    let mut html_out = String::with_capacity(src.len() * 2);
    md_html::push_html(&mut html_out, buffered.into_iter());
    sanitize_markdown_html(&html_out)
}

fn html_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

/// Static metadata for a [`PolicyKind`] hero banner: icon glyph, title key,
/// and short subtitle key.
struct PolicyHero {
    icon: icondata::Icon,
    title_key: I18nKey,
    subtitle_key: I18nKey,
    modifier: &'static str,
}

fn policy_hero(kind: PolicyKind) -> PolicyHero {
    match kind {
        PolicyKind::License => PolicyHero {
            icon: icondata::LuScale,
            title_key: I18nKey::FilePreviewPolicyLicenseTitle,
            subtitle_key: I18nKey::FilePreviewPolicyLicenseSubtitle,
            modifier: "license",
        },
        PolicyKind::Contributing => PolicyHero {
            icon: icondata::LuGitPullRequest,
            title_key: I18nKey::FilePreviewPolicyContributingTitle,
            subtitle_key: I18nKey::FilePreviewPolicyContributingSubtitle,
            modifier: "contributing",
        },
        PolicyKind::Contributors => PolicyHero {
            icon: icondata::LuUsers,
            title_key: I18nKey::FilePreviewPolicyContributorsTitle,
            subtitle_key: I18nKey::FilePreviewPolicyContributorsSubtitle,
            modifier: "contributors",
        },
        PolicyKind::CodeOfConduct => PolicyHero {
            icon: icondata::LuShieldCheck,
            title_key: I18nKey::FilePreviewPolicyCodeOfConductTitle,
            subtitle_key: I18nKey::FilePreviewPolicyCodeOfConductSubtitle,
            modifier: "code-of-conduct",
        },
        PolicyKind::Security => PolicyHero {
            icon: icondata::LuLock,
            title_key: I18nKey::FilePreviewPolicySecurityTitle,
            subtitle_key: I18nKey::FilePreviewPolicySecuritySubtitle,
            modifier: "security",
        },
        PolicyKind::Authors => PolicyHero {
            icon: icondata::LuUserRound,
            title_key: I18nKey::FilePreviewPolicyAuthorsTitle,
            subtitle_key: I18nKey::FilePreviewPolicyAuthorsSubtitle,
            modifier: "authors",
        },
        PolicyKind::Changelog => PolicyHero {
            icon: icondata::LuHistory,
            title_key: I18nKey::FilePreviewPolicyChangelogTitle,
            subtitle_key: I18nKey::FilePreviewPolicyChangelogSubtitle,
            modifier: "changelog",
        },
        PolicyKind::Readme => PolicyHero {
            icon: icondata::LuBookOpen,
            title_key: I18nKey::FilePreviewPolicyReadmeTitle,
            subtitle_key: I18nKey::FilePreviewPolicyReadmeSubtitle,
            modifier: "readme",
        },
        PolicyKind::Support => PolicyHero {
            icon: icondata::LuLifeBuoy,
            title_key: I18nKey::FilePreviewPolicySupportTitle,
            subtitle_key: I18nKey::FilePreviewPolicySupportSubtitle,
            modifier: "support",
        },
        PolicyKind::Agents => PolicyHero {
            icon: icondata::LuBot,
            title_key: I18nKey::FilePreviewPolicyAgentsTitle,
            subtitle_key: I18nKey::FilePreviewPolicyAgentsSubtitle,
            modifier: "agents",
        },
        PolicyKind::Claude => PolicyHero {
            icon: icondata::LuSparkles,
            title_key: I18nKey::FilePreviewPolicyClaudeTitle,
            subtitle_key: I18nKey::FilePreviewPolicyClaudeSubtitle,
            modifier: "claude",
        },
        PolicyKind::Codex => PolicyHero {
            icon: icondata::LuCode,
            title_key: I18nKey::FilePreviewPolicyCodexTitle,
            subtitle_key: I18nKey::FilePreviewPolicyCodexSubtitle,
            modifier: "codex",
        },
        PolicyKind::Gemini => PolicyHero {
            icon: icondata::LuSparkles,
            title_key: I18nKey::FilePreviewPolicyGeminiTitle,
            subtitle_key: I18nKey::FilePreviewPolicyGeminiSubtitle,
            modifier: "gemini",
        },
    }
}

#[component]
pub fn MarkdownView(
    workspace_id: u64,
    rel_path: String,
    reload_tick: ReadSignal<u32>,
    #[prop(default = None)] policy_kind: Option<PolicyKind>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let html_sig = RwSignal::new(None::<Result<String, FilePreviewError>>);
    let node_ref: NodeRef<html::Article> = NodeRef::new();
    let mermaid_err = RwSignal::new(false);

    let rel_for_effect = rel_path.clone();
    Effect::new(move |_| {
        // Only refetch on explicit reload. See FilePreviewDock for context.
        let _ = reload_tick.get();
        html_sig.set(None);
        mermaid_err.set(false);
        if !is_tauri_shell() {
            html_sig.set(Some(Err(FilePreviewError::NoTauri)));
            return;
        }
        let Some(root) = wb.workspaces().with_untracked(|list| {
            list.iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.cwd.clone())
        }) else {
            html_sig.set(Some(Err(FilePreviewError::WorkspaceNotFound)));
            return;
        };
        let rel = rel_for_effect.clone();
        spawn_local(async move {
            match read_workspace_text_file(root, rel).await {
                Ok(t) => {
                    let rendered = render_markdown(&t.content);
                    html_sig.set(Some(Ok(rendered)));
                }
                Err(e) => html_sig.set(Some(Err(FilePreviewError::Failed(e)))),
            }
        });
    });

    // After the HTML is mounted, hunt for `.mermaid` sentinels and run the lib.
    Effect::new(move |_| {
        if !matches!(html_sig.get(), Some(Ok(_))) {
            return;
        }
        let Some(el) = node_ref.get() else {
            return;
        };
        let container: HtmlElement = el.unchecked_into();
        let Ok(node_list) = container.query_selector_all(".mermaid") else {
            return;
        };
        let mut nodes = Vec::with_capacity(node_list.length() as usize);
        for i in 0..node_list.length() {
            if let Some(node) = node_list.item(i) {
                if let Ok(el) = node.dyn_into::<Element>() {
                    if let Ok(html_el) = el.dyn_into::<HtmlElement>() {
                        nodes.push(html_el);
                    }
                }
            }
        }
        if nodes.is_empty() {
            return;
        }
        spawn_local(async move {
            if let Err(e) = run_mermaid_on(&nodes).await {
                web_sys::console::warn_1(&format!("mermaid render: {e}").into());
                mermaid_err.set(true);
            }
        });
    });

    let hero = policy_kind.map(policy_hero);
    let hero_class = hero
        .as_ref()
        .map(|h| {
            format!(
                "file-preview__stage file-preview__stage--markdown file-preview__stage--policy file-preview__stage--policy-{}",
                h.modifier
            )
        })
        .unwrap_or_else(|| "file-preview__stage file-preview__stage--markdown".to_string());

    view! {
        <div class=hero_class>
            {hero.map(|h| view! {
                <header class=format!("file-preview__policy-hero file-preview__policy-hero--{}", h.modifier)>
                    <span class="file-preview__policy-hero__icon" aria-hidden="true">
                        <LxIcon icon=h.icon width="1.4rem" height="1.4rem" />
                    </span>
                    <div class="file-preview__policy-hero__text">
                        <h1 class="file-preview__policy-hero__title">
                            {move || i18n.tr(h.title_key)()}
                        </h1>
                        <p class="file-preview__policy-hero__subtitle">
                            {move || i18n.tr(h.subtitle_key)()}
                        </p>
                    </div>
                </header>
            })}
            {move || match html_sig.get() {
                None => view! {
                    <div class="file-preview__status">{i18n.tr(I18nKey::FilePreviewLoading)}</div>
                }.into_any(),
                Some(Err(err)) => render_load_error(i18n, I18nKey::FilePreviewLoadFailedMarkdown, err),
                Some(Ok(html_content)) => view! {
                    <article
                        node_ref=node_ref
                        class="file-preview__markdown"
                        inner_html=html_content
                    />
                    <Show when=move || mermaid_err.get()>
                        <div class="file-preview__notice file-preview__notice--error">
                            {i18n.tr(I18nKey::FilePreviewMermaidError)}
                        </div>
                    </Show>
                }.into_any(),
            }}
        </div>
    }
}
