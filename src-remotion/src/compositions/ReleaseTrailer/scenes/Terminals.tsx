import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background } from "../components/Background";
import { FeatureCard } from "../components/FeatureCard";

const DURATION = 150;
const FEATURE_ENTER = 18;

const TERMINALS: { name: string; slot: string; prompt: string; accent: string; delay: number }[] = [
  { name: "Devon", slot: "1", prompt: "$ cargo test -p blxcode-ui", accent: "#bd93f9", delay: 0 },
  { name: "Tom", slot: "2", prompt: "$ git push origin feature/ptt", accent: "#7dcfff", delay: 0.1 },
  { name: "Mia", slot: "3", prompt: "$ remotion render Trailer out.mp4", accent: "#ff79c6", delay: 0.2 },
];

const Terminal: React.FC<{
  name: string;
  slot: string;
  prompt: string;
  accent: string;
  x: number;
  y: number;
  index: number;
}> = ({ name, slot, prompt, accent, x, y, index }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const enter = spring({
    frame: frame - 15 - index * 6,
    fps,
    config: { damping: 16, stiffness: 100, mass: 0.7 },
  });
  const enterY = (1 - enter) * 30;

  // Morph from #N to name
  const morphFrame = 30 + index * 8;
  const morphT = interpolate(frame, [morphFrame, morphFrame + 12], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  // Type prompt
  const typedLen = Math.min(prompt.length, Math.max(0, Math.floor((frame - (morphFrame + 8)) * 1.4)));

  return (
    <div
      style={{
        position: "absolute",
        left: x,
        top: y,
        width: 380,
        opacity: enter,
        transform: `translateY(${enterY}px)`,
      }}
    >
      <div
        style={{
          background: "var(--bg-panel)",
          border: `1px solid ${accent}40`,
          borderRadius: 14,
          overflow: "hidden",
          boxShadow: `0 20px 60px -20px ${accent}33, 0 0 0 1px ${accent}1a inset`,
        }}
      >
        {/* Titlebar */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 10,
            padding: "10px 14px",
            background: "var(--bg-panel-header)",
            borderBottom: "1px solid var(--border)",
          }}
        >
          <span style={{ width: 8, height: 8, borderRadius: "50%", background: accent, boxShadow: `0 0 8px ${accent}` }} />
          <div style={{ position: "relative", width: 100, height: 20 }}>
            <span
              style={{
                position: "absolute",
                inset: 0,
                fontFamily: "var(--font-mono)",
                fontSize: 16,
                color: "var(--text-faint)",
                opacity: 1 - morphT,
                transform: `translateX(${(1 - morphT) * -8}px)`,
              }}
            >
              #{slot}
            </span>
            <span
              style={{
                position: "absolute",
                inset: 0,
                fontFamily: "var(--font-sans)",
                fontSize: 18,
                fontWeight: 600,
                color: accent,
                opacity: morphT,
                transform: `translateX(${(1 - morphT) * 8}px)`,
              }}
            >
              {name}
            </span>
          </div>
        </div>
        {/* Body */}
        <div
          style={{
            padding: 18,
            fontFamily: "var(--font-mono)",
            fontSize: 16,
            color: "var(--text-muted)",
            background: "var(--bg-raised)",
            minHeight: 110,
          }}
        >
          {prompt.slice(0, typedLen)}
          <span
            style={{
              display: "inline-block",
              width: 8,
              height: 16,
              background: accent,
              marginLeft: 2,
              verticalAlign: "text-bottom",
              opacity: Math.sin(frame * 0.6) * 0.4 + 0.6,
            }}
          />
        </div>
      </div>
    </div>
  );
};

export const TerminalsScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const exit = spring({
    frame: frame - (DURATION - 18),
    fps,
    config: { damping: 24, stiffness: 100, mass: 0.7 },
  });
  const exitOpacity = Math.max(0, 1 - Math.max(0, exit - 1));

  return (
    <AbsoluteFill className="terminals-scene" style={{ opacity: exitOpacity }}>
      <Background fadeIn={0} />
      <AbsoluteFill style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between", padding: "0 100px" }}>
        <FeatureCard
          feature={{
            id: "terminals",
            kicker: "Workspaces",
            title: "Named terminals",
            body: "Devon, Tom, Mia — friendly names with deterministic slot mapping. Double-click a terminal header to rename. The Agent knows the names too.",
            accent: "warning",
          }}
          enterAt={FEATURE_ENTER}
          durationInFrames={DURATION}
          side="left"
        />

        <div style={{ position: "relative", width: 480, height: 540 }}>
          {TERMINALS.map((t, i) => (
            <Terminal
              key={t.name}
              {...t}
              x={i * 40}
              y={i * 110}
              index={i}
            />
          ))}

          {/* Subtitle */}
          <div
            style={{
              position: "absolute",
              bottom: -40,
              left: 0,
              right: 0,
              textAlign: "center",
              fontFamily: "var(--font-mono)",
              fontSize: 16,
              color: "var(--text-faint)",
              letterSpacing: "0.08em",
              opacity: interpolate(frame, [80, 100], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }),
            }}
          >
            tell the agent: &ldquo;ask Devon to run the tests&rdquo;
          </div>
        </div>
      </AbsoluteFill>
      <style>{`
        .terminals-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

export const TERMINALS_DURATION = DURATION;
