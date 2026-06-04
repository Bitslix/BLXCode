import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background, FeatureCard } from "../components";

const DURATION = 120;
const FEATURE_ENTER = 12;

type Change = { path: string; added: number; removed: number; delay: number };

const CHANGES: Change[] = [
  { path: "src-tauri/src/agent/timeline.rs", added: 184, removed: 12, delay: 24 },
  { path: "src-tauri/src/agent/changed_files.rs", added: 220, removed: 0, delay: 30 },
  { path: "src/agent_panel/changed_files_card", added: 311, removed: 0, delay: 36 },
  { path: "src/agent_panel/tool_group", added: 198, removed: 0, delay: 42 },
  { path: "src/agent_panel/composer/mod.rs", added: 142, removed: 38, delay: 48 },
  { path: "src/i18n/locales/en_us.rs", added: 38, removed: 0, delay: 54 },
];

export const ChangedFilesScene: React.FC = () => {
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

  const totalAdded = CHANGES.reduce((a, c) => a + c.added, 0);
  const totalRemoved = CHANGES.reduce((a, c) => a + c.removed, 0);
  const counterAdded = Math.floor(
    interpolate(frame, [10, 50], [0, totalAdded], { extrapolateLeft: "clamp", extrapolateRight: "clamp" })
  );
  const counterRemoved = Math.floor(
    interpolate(frame, [10, 50], [0, totalRemoved], { extrapolateLeft: "clamp", extrapolateRight: "clamp" })
  );

  return (
    <AbsoluteFill className="changed-scene" style={{ opacity: exitOpacity }}>
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
            id: "changed-files",
            kicker: "Timeline",
            title: "Changed-files summary card",
            body: "Model rounds that mutate files end with a totals card and a collapsible directory tree. Click any file to open the diff view.",
            accent: "success",
          }}
          enterAt={FEATURE_ENTER}
          durationInFrames={DURATION}
          side="left"
        />

        <div
          style={{
            width: 700,
            opacity: cardEnter,
            transform: `translateY(${(1 - cardEnter) * 20}px)`,
            background: "var(--bg-panel)",
            border: "1px solid var(--border-strong)",
            borderRadius: 20,
            padding: 24,
            boxShadow: "0 30px 80px -20px rgba(0, 0, 0, 0.5)",
          }}
        >
          {/* Header with totals */}
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 16,
              paddingBottom: 16,
              borderBottom: "1px solid var(--border)",
            }}
          >
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                fontFamily: "var(--font-mono)",
                fontSize: 14,
                color: "var(--text-faint)",
                letterSpacing: "0.1em",
                textTransform: "uppercase",
              }}
            >
              <span
                style={{
                  width: 8,
                  height: 8,
                  borderRadius: "50%",
                  background: "var(--status-success)",
                  boxShadow: "0 0 8px var(--status-success)",
                }}
              />
              changed files
            </div>
            <div style={{ flex: 1 }} />
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 16,
                fontFamily: "var(--font-mono)",
                fontSize: 18,
                fontWeight: 600,
                fontFeatureSettings: '"tnum"',
              }}
            >
              <span style={{ color: "var(--status-success)" }}>+{counterAdded}</span>
              <span style={{ color: "var(--status-danger)" }}>-{counterRemoved}</span>
            </div>
          </div>

          {/* Tree */}
          <div style={{ marginTop: 18, fontFamily: "var(--font-mono)", fontSize: 16 }}>
            {CHANGES.map((change) => {
              const enter = spring({
                frame: frame - change.delay,
                fps,
                config: { damping: 16, stiffness: 120, mass: 0.5 },
              });
              const indent = (change.path.match(/\//g) ?? []).length * 20;
              const isNew = change.removed === 0;
              return (
                <div
                  key={change.path}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 12,
                    padding: "5px 0",
                    paddingLeft: indent,
                    opacity: enter,
                    transform: `translateX(${(1 - enter) * 12}px)`,
                  }}
                >
                  <span
                    style={{
                      color: "var(--text-faint)",
                      width: 14,
                    }}
                  >
                    {isNew ? "✦" : "▸"}
                  </span>
                  <span
                    style={{
                      flex: 1,
                      color: "var(--text)",
                      whiteSpace: "nowrap",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                    }}
                  >
                    {change.path}
                  </span>
                  <span
                    style={{
                      color: "var(--status-success)",
                      width: 60,
                      textAlign: "right",
                      fontFeatureSettings: '"tnum"',
                    }}
                  >
                    +{change.added}
                  </span>
                  <span
                    style={{
                      color: change.removed > 0 ? "var(--status-danger)" : "var(--text-faint)",
                      width: 50,
                      textAlign: "right",
                      fontFeatureSettings: '"tnum"',
                      opacity: change.removed > 0 ? 1 : 0.3,
                    }}
                  >
                    -{change.removed}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      </AbsoluteFill>
      <style>{`
        .changed-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

export const CHANGED_FILES_DURATION = DURATION;
