import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background, FeatureCard } from "../components";

const DURATION = 120;
const FEATURE_ENTER = 12;

const MeterBar: React.FC<{ pct: number; delay: number }> = ({ pct, delay }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const enter = spring({
    frame: frame - delay,
    fps,
    config: { damping: 18, stiffness: 90, mass: 0.7 },
  });
  const fill = interpolate(frame, [delay, delay + 30], [0, pct], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  let color = "var(--status-success)";
  if (pct >= 85) color = "var(--status-danger)";
  else if (pct >= 70) color = "var(--status-warning)";
  return (
    <div
      style={{
        position: "relative",
        height: 4,
        borderRadius: 2,
        background: "var(--bg-raised)",
        overflow: "hidden",
        opacity: enter,
      }}
    >
      <div
        style={{
          position: "absolute",
          inset: 0,
          width: `${fill}%`,
          background: color,
          borderRadius: 2,
          transition: "width 0.2s",
        }}
      />
    </div>
  );
};

const StatRow: React.FC<{ label: string; value: string; accent?: string; delay: number }> = ({
  label,
  value,
  accent,
  delay,
}) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const enter = spring({
    frame: frame - delay,
    fps,
    config: { damping: 16, stiffness: 110, mass: 0.55 },
  });
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 14,
        padding: "10px 0",
        opacity: enter,
        transform: `translateX(${(1 - enter) * 12}px)`,
      }}
    >
      <div
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 12,
          color: "var(--text-faint)",
          letterSpacing: "0.12em",
          textTransform: "uppercase",
          width: 130,
        }}
      >
        {label}
      </div>
      <div
        style={{
          fontFamily: "var(--font-sans)",
          fontSize: 18,
          fontWeight: 500,
          color: accent ?? "var(--text-bright)",
          fontFeatureSettings: '"tnum"',
        }}
      >
        {value}
      </div>
    </div>
  );
};

export const StatsScene: React.FC = () => {
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

  // Auto-compact threshold pulse
  const compactPulse = frame > 75 && frame < 105;
  const pulse = compactPulse ? Math.sin((frame - 75) * 0.8) * 0.5 + 0.5 : 0;

  return (
    <AbsoluteFill className="stats-scene" style={{ opacity: exitOpacity }}>
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
            id: "stats",
            kicker: "Agent",
            title: "Session stats + context meter",
            body: "Live provider/model chip, session start, context used/max with a thin meter that warns at 70% and danders at 85%, total turns and cost.",
            accent: "cyan",
          }}
          enterAt={FEATURE_ENTER}
          durationInFrames={DURATION}
          side="left"
        />

        <div
          style={{
            width: 660,
            opacity: cardEnter,
            transform: `translateY(${(1 - cardEnter) * 20}px) scale(${0.96 + 0.04 * cardEnter})`,
            background: "var(--bg-panel)",
            border: "1px solid var(--border-strong)",
            borderRadius: 20,
            padding: 28,
            boxShadow: "0 30px 80px -20px rgba(0, 0, 0, 0.5)",
          }}
        >
          {/* Header */}
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 12,
              marginBottom: 18,
              paddingBottom: 16,
              borderBottom: "1px solid var(--border)",
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
              Session
            </div>
            <div style={{ flex: 1 }} />
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 6,
                padding: "4px 10px",
                borderRadius: 999,
                background: "var(--accent-purple-soft)",
                border: "1px solid var(--accent-purple-soft)",
                fontFamily: "var(--font-mono)",
                fontSize: 12,
                color: "var(--accent-purple)",
              }}
            >
              <span
                style={{
                  width: 6,
                  height: 6,
                  borderRadius: "50%",
                  background: "var(--accent-purple)",
                  boxShadow: "0 0 6px var(--accent-purple)",
                }}
              />
              thinking
            </div>
          </div>

          {/* Model / provider button */}
          <StatRow
            label="Model"
            value="claude · opus 4.6"
            accent="var(--text-bright)"
            delay={10}
          />
          <StatRow label="Provider" value="Anthropic" delay={14} />
          <StatRow label="Started" value="11:42:08" delay={18} />
          <StatRow
            label="Turns"
            value="User 12 / Model 12"
            accent="var(--text)"
            delay={22}
          />
          <StatRow label="Tool calls" value="48" delay={26} />
          <StatRow label="Subagents" value="Devon, Tom" delay={30} />
          <StatRow label="Cost" value="$0.214" delay={34} />

          {/* Context meter */}
          <div style={{ marginTop: 14, paddingTop: 14, borderTop: "1px solid var(--border)" }}>
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 14,
                marginBottom: 8,
              }}
            >
              <div
                style={{
                  fontFamily: "var(--font-mono)",
                  fontSize: 12,
                  color: "var(--text-faint)",
                  letterSpacing: "0.12em",
                  textTransform: "uppercase",
                  width: 130,
                }}
              >
                Context
              </div>
              <div
                style={{
                  fontFamily: "var(--font-mono)",
                  fontSize: 14,
                  color: "var(--text-bright)",
                  fontFeatureSettings: '"tnum"',
                }}
              >
                142.4k / 200.0k · 71%
              </div>
            </div>
            <MeterBar pct={71} delay={40} />
            <div
              style={{
                marginTop: 10,
                fontFamily: "var(--font-mono)",
                fontSize: 12,
                color: "var(--text-faint)",
                display: "flex",
                alignItems: "center",
                gap: 8,
              }}
            >
              <span
                style={{
                  display: "inline-block",
                  width: 8,
                  height: 8,
                  borderRadius: "50%",
                  background: `rgba(255, 184, 108, ${0.6 + 0.4 * pulse})`,
                  boxShadow: `0 0 ${4 + 4 * pulse}px var(--status-warning)`,
                }}
              />
              auto-compact at 85%
            </div>
          </div>
        </div>
      </AbsoluteFill>
      <style>{`
        .stats-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

export const STATS_DURATION = DURATION;
