import { AbsoluteFill, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background, FeatureCard } from "../components";

const DURATION = 120;
const FEATURE_ENTER = 12;

type Entry = { title: string; category: string; source: string; match: "title" | "desc" | "category"; delay: number; active: boolean };

const ENTRIES: Entry[] = [
  { title: "voice: push-to-talk", category: "voice", source: ".agents/skills", match: "category", delay: 22, active: true },
  { title: "github: pr-review-toolkit", category: "github", source: ".agents/skills", match: "title", delay: 28, active: true },
  { title: "memory: workspace index", category: "memory", source: ".agents/skills", match: "title", delay: 34, active: true },
  { title: "rust: idiomatic patterns", category: "rust", source: ".agents/skills", match: "desc", delay: 40, active: false },
  { title: "tts: voice runtime state", category: "voice", source: ".agents/skills", match: "category", delay: 46, active: true },
  { title: "review: code review playbook", category: "review", source: ".agents/skills", match: "title", delay: 52, active: false },
];

const FILTERS: { id: string; label: string; active: boolean }[] = [
  { id: "all", label: "All", active: true },
  { id: "voice", label: "voice", active: true },
  { id: "github", label: "github", active: true },
  { id: "memory", label: "memory", active: true },
  { id: "rust", label: "rust", active: false },
  { id: "review", label: "review", active: false },
];

export const FilterScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const exit = spring({
    frame: frame - (DURATION - 18),
    fps,
    config: { damping: 24, stiffness: 100, mass: 0.7 },
  });
  const exitOpacity = Math.max(0, 1 - Math.max(0, exit - 1));

  const cardEnter = spring({
    frame: frame - 4,
    fps,
    config: { damping: 18, stiffness: 90, mass: 0.7 },
  });

  const searchActive = frame > 16;
  const query = "voi";
  const queryChars = searchActive
    ? Math.min(query.length, Math.floor((frame - 16) * 0.4))
    : 0;
  const queryDisplay = query.slice(0, queryChars);

  return (
    <AbsoluteFill className="filter-scene" style={{ opacity: exitOpacity }}>
      <Background fadeIn={0} />
      <AbsoluteFill
        style={{
          flexDirection: "row",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "0 100px",
        }}
      >
        <FeatureCard
          feature={{
            id: "filter",
            kicker: "Rules · Skills · Plans",
            title: "Unified category + live search",
            body: "Rules and skills share the same filter row, themed separator, and search. The Plans panel adds live search on top of its status tabs.",
            accent: "purple",
          }}
          enterAt={FEATURE_ENTER}
          durationInFrames={DURATION}
          side="left"
        />

        <div
          style={{
            width: 720,
            opacity: cardEnter,
            transform: `translateY(${(1 - cardEnter) * 20}px)`,
            background: "var(--bg-panel)",
            border: "1px solid var(--border-strong)",
            borderRadius: 20,
            overflow: "hidden",
            boxShadow: "0 30px 80px -20px rgba(0, 0, 0, 0.5)",
          }}
        >
          {/* Header */}
          <div
            style={{
              padding: "14px 20px",
              borderBottom: "1px solid var(--border)",
              display: "flex",
              alignItems: "center",
              gap: 12,
            }}
          >
            <div
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: 12,
                color: "var(--text-faint)",
                letterSpacing: "0.14em",
                textTransform: "uppercase",
              }}
            >
              Skills · 14
            </div>
            <div style={{ flex: 1 }} />
            {/* Search */}
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                padding: "6px 12px",
                borderRadius: 8,
                background: "var(--bg-raised)",
                border: searchActive ? "1px solid var(--accent-purple)" : "1px solid var(--border)",
                width: 200,
              }}
            >
              <span style={{ fontSize: 14, color: "var(--text-faint)" }}>⌕</span>
              <span
                style={{
                  fontFamily: "var(--font-mono)",
                  fontSize: 14,
                  color: "var(--text-bright)",
                  flex: 1,
                }}
              >
                {queryDisplay}
                {searchActive && (
                  <span
                    style={{
                      display: "inline-block",
                      width: 2,
                      height: 16,
                      background: "var(--accent-purple)",
                      marginLeft: 2,
                      verticalAlign: "text-bottom",
                      opacity: Math.sin(frame * 0.5) * 0.4 + 0.6,
                    }}
                  />
                )}
              </span>
            </div>
          </div>

          {/* Category filters */}
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              padding: "10px 20px",
              borderBottom: "1px solid var(--border)",
              background: "var(--bg-panel-header)",
            }}
          >
            {FILTERS.map((f) => {
              const enter = spring({
                frame: frame - 12,
                fps,
                config: { damping: 16, stiffness: 130, mass: 0.5 },
              });
              return (
                <div
                  key={f.id}
                  style={{
                    padding: "4px 10px",
                    borderRadius: 999,
                    background: f.active ? "var(--accent-purple-soft)" : "transparent",
                    border: `1px solid ${f.active ? "var(--accent-purple)" : "var(--border)"}`,
                    fontFamily: "var(--font-mono)",
                    fontSize: 12,
                    color: f.active ? "var(--accent-purple)" : "var(--text-faint)",
                    fontWeight: f.active ? 600 : 400,
                    opacity: enter,
                  }}
                >
                  {f.label}
                </div>
              );
            })}
          </div>

          {/* Entries */}
          <div>
            {ENTRIES.map((e, i) => {
              const enter = spring({
                frame: frame - e.delay,
                fps,
                config: { damping: 16, stiffness: 130, mass: 0.5 },
              });
              return (
                <div
                  key={e.title}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 14,
                    padding: "12px 20px",
                    borderBottom: i < ENTRIES.length - 1 ? "1px solid var(--border)" : "none",
                    opacity: enter,
                    transform: `translateX(${(1 - enter) * 16}px)`,
                  }}
                >
                  <span style={{ fontSize: 16, color: "var(--text-muted)" }}>▢</span>
                  <span
                    style={{
                      flex: 1,
                      fontSize: 16,
                      color: "var(--text-bright)",
                      fontFamily: "var(--font-sans)",
                    }}
                  >
                    {e.title}
                  </span>
                  <span
                    style={{
                      padding: "2px 8px",
                      borderRadius: 999,
                      background: e.active ? "var(--accent-purple-soft)" : "var(--bg-raised)",
                      border: `1px solid ${e.active ? "var(--accent-purple)" : "var(--border)"}`,
                      fontFamily: "var(--font-mono)",
                      fontSize: 11,
                      color: e.active ? "var(--accent-purple)" : "var(--text-faint)",
                      textTransform: "uppercase",
                      letterSpacing: "0.1em",
                    }}
                  >
                    {e.category}
                  </span>
                  <span
                    style={{
                      fontFamily: "var(--font-mono)",
                      fontSize: 11,
                      color: "var(--text-faint)",
                    }}
                  >
                    {e.source}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      </AbsoluteFill>
      <style>{`
        .filter-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

export const FILTER_DURATION = DURATION;
