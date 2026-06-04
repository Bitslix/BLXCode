import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background } from "../components/Background";
import { FeatureCard } from "../components/FeatureCard";

const DURATION = 150;
const FEATURE_ENTER = 18;

type Commit = {
  id: string;
  message: string;
  author: string;
  lane: number;
  parents: number[];
  isSelected?: boolean;
  hasExpand?: boolean;
};

const COMMITS: Commit[] = [
  { id: "f3a2c1", message: "feat: agent timeline refactor", author: "iptoux", lane: 0, parents: [1, 2] },
  { id: "8b4e22", message: "feat: 3d drobo agent orb", author: "iptoux", lane: 0, parents: [3] },
  { id: "9c1a0d", message: "fix: composer send/stop toggle", author: "iptoux", lane: 1, parents: [3] },
  { id: "2a0f88", message: "feat: redesigned blxcode theme", author: "iptoux", lane: 0, parents: [4] },
  { id: "71bc40", message: "feat: push-to-talk with whisper", author: "iptoux", lane: 2, parents: [5], isSelected: true, hasExpand: true },
  { id: "5e2d39", message: "feat: ai plan / ai tasks", author: "iptoux", lane: 0, parents: [-1] },
];

const LANE_COLORS = ["#bd93f9", "#7dcfff", "#50fa7b", "#ffb86c", "#ff5555", "#7aa2f7"];

export const GitGraphScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const exit = spring({
    frame: frame - (DURATION - 18),
    fps,
    config: { damping: 24, stiffness: 100, mass: 0.7 },
  });
  const exitOpacity = Math.max(0, 1 - Math.max(0, exit - 1));

  const graphEnter = interpolate(frame, [8, 28], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const graphY = interpolate(frame, [8, 28], [30, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  // Stagger reveal of commits
  const commitReveal = (i: number) => interpolate(frame, [15 + i * 6, 25 + i * 6], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  // Selected commit expand
  const expandProgress = interpolate(frame, [80, 110], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  const rowH = 56;
  const laneX = 40;
  const commitStartY = 80;

  return (
    <AbsoluteFill className="git-scene" style={{ opacity: exitOpacity }}>
      <Background fadeIn={0} />
      <AbsoluteFill style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between", padding: "0 100px" }}>
        <div
          style={{
            width: 680,
            opacity: graphEnter,
            transform: `translateY(${graphY}px)`,
            background: "var(--bg-panel)",
            border: "1px solid var(--border-strong)",
            borderRadius: 20,
            padding: 24,
            boxShadow: "0 30px 80px -20px rgba(0, 0, 0, 0.5)",
          }}
        >
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 10,
              marginBottom: 20,
              fontFamily: "var(--font-mono)",
              fontSize: 16,
              color: "var(--text-muted)",
              letterSpacing: "0.08em",
              textTransform: "uppercase",
            }}
          >
            <span style={{ width: 8, height: 8, borderRadius: "50%", background: "var(--status-success)" }} />
            git commits
          </div>

          <div style={{ position: "relative" }}>
            {/* Lane lines */}
            {LANE_COLORS.map((color, i) => (
              <div
                key={i}
                style={{
                  position: "absolute",
                  left: laneX + i * 22,
                  top: 0,
                  bottom: 0,
                  width: 2,
                  background: `${color}33`,
                }}
              />
            ))}

            {COMMITS.map((commit, i) => {
              const reveal = commitReveal(i);
              const y = commitStartY + i * rowH;
              const x = laneX + commit.lane * 22 - 6;
              return (
                <div
                  key={commit.id}
                  style={{
                    position: "relative",
                    height: rowH,
                    display: "flex",
                    alignItems: "center",
                    gap: 16,
                    paddingLeft: laneX + 6 * 22 + 20,
                    opacity: reveal,
                    transform: `translateX(${(1 - reveal) * 20}px)`,
                  }}
                >
                  {/* Node */}
                  <div
                    style={{
                      position: "absolute",
                      left: x,
                      top: y - commitStartY + rowH / 2 - 7,
                      width: 14,
                      height: 14,
                      borderRadius: "50%",
                      background: commit.isSelected ? "#f1fa8c" : LANE_COLORS[commit.lane],
                      border: commit.isSelected ? "2px solid var(--text-bright)" : "none",
                      boxShadow: commit.isSelected
                        ? "0 0 16px #f1fa8c"
                        : `0 0 8px ${LANE_COLORS[commit.lane]}66`,
                      zIndex: 2,
                    }}
                  />
                  {/* SHA */}
                  <span
                    style={{
                      fontFamily: "var(--font-mono)",
                      fontSize: 14,
                      color: "var(--text-faint)",
                      width: 70,
                    }}
                  >
                    {commit.id}
                  </span>
                  <span
                    style={{
                      fontFamily: "var(--font-sans)",
                      fontSize: 18,
                      color: commit.isSelected ? "var(--text-bright)" : "var(--text)",
                      fontWeight: commit.isSelected ? 600 : 400,
                      flex: 1,
                      whiteSpace: "nowrap",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                    }}
                  >
                    {commit.message}
                  </span>
                  {commit.hasExpand && (
                    <span
                      style={{
                        fontSize: 14,
                        color: "var(--text-faint)",
                        transform: `rotate(${expandProgress * 90}deg)`,
                        transition: "transform 0.3s",
                      }}
                    >
                      ▾
                    </span>
                  )}
                </div>
              );
            })}
          </div>

          {/* Expanded file list */}
          <div
            style={{
              maxHeight: expandProgress * 140,
              overflow: "hidden",
              marginTop: 8,
              padding: expandProgress > 0.5 ? "12px 0 0 90px" : 0,
            }}
          >
            {["voice/ptt_runtime.rs", "voice/stt/mod.rs", "voice/models.rs"].map((f, i) => (
              <div
                key={f}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 12,
                  padding: "4px 0",
                  fontFamily: "var(--font-mono)",
                  fontSize: 14,
                  color: "var(--text-muted)",
                  opacity: expandProgress,
                }}
              >
                <span style={{ color: "var(--status-success)" }}>+{120 - i * 30}</span>
                <span style={{ color: "var(--status-danger)" }}>-{20 + i * 8}</span>
                <span>{f}</span>
              </div>
            ))}
          </div>
        </div>

        <FeatureCard
          feature={{
            id: "git",
            kicker: "Git",
            title: "VS Code-style commit graph",
            body: "Colored lanes, click-to-expand file lists, hover cards with author, date, refs, and Open on GitHub.",
            accent: "success",
          }}
          enterAt={FEATURE_ENTER}
          durationInFrames={DURATION}
          side="right"
        />
      </AbsoluteFill>
      <style>{`
        .git-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

export const GITGRAPH_DURATION = DURATION;
