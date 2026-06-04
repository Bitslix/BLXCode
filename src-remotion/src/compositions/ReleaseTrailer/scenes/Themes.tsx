import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background } from "../components/Background";
import { FeatureCard } from "../components/FeatureCard";

const DURATION = 150;
const FEATURE_ENTER = 18;

type Theme = { id: string; name: string; bg: string; accent: string; text: string; dark: boolean };

const THEMES: Theme[] = [
  { id: "blxcode", name: "BLXCode", bg: "#16161e", accent: "#bd93f9", text: "#c8d3f5", dark: true },
  { id: "blxcode-light", name: "BLXCode Light", bg: "#e6e7f0", accent: "#8839ef", text: "#3a3a5c", dark: false },
  { id: "blxcode-legacy", name: "BLXCode Legacy", bg: "#090a0d", accent: "#58a6ff", text: "#f1f2f5", dark: true },
  { id: "claude-code", name: "Claude Code", bg: "#1f1e1d", accent: "#d97757", text: "#e8e6e3", dark: true },
  { id: "tokyo-night", name: "Tokyo Night", bg: "#1a1b26", accent: "#7aa2f7", text: "#c0caf5", dark: true },
  { id: "tokyo-night-light", name: "Tokyo Night Light", bg: "#d5d6db", accent: "#2e7de9", text: "#34394e", dark: false },
  { id: "dracula", name: "Dracula", bg: "#282a36", accent: "#bd93f9", text: "#f8f8f2", dark: true },
  { id: "nord", name: "Nord", bg: "#2e3440", accent: "#88c0d0", text: "#eceff4", dark: true },
  { id: "nord-light", name: "Nord Light", bg: "#eceff4", accent: "#5e81ac", text: "#2e3440", dark: false },
  { id: "winter", name: "Winter", bg: "#0d1b2a", accent: "#7dcfff", text: "#e0fbfc", dark: true },
  { id: "winter-light", name: "Winter Light", bg: "#e6f7ff", accent: "#1a8fb8", text: "#0a2540", dark: false },
  { id: "paper", name: "Paper", bg: "#f4ecd8", accent: "#2649a0", text: "#2a2622", dark: false },
  { id: "alpine", name: "Alpine", bg: "#0c1f1f", accent: "#5fb3a1", text: "#dff5ee", dark: true },
  { id: "frost", name: "Frost", bg: "#ffffff", accent: "#3b82c4", text: "#0c2233", dark: false },
  { id: "lilac", name: "Lilac", bg: "#f3eaff", accent: "#7c3aed", text: "#2a1f4d", dark: false },
  { id: "ayu", name: "Ayu", bg: "#0f1419", accent: "#e6b450", text: "#bfbdb6", dark: true },
];

export const ThemesScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const exit = spring({
    frame: frame - (DURATION - 18),
    fps,
    config: { damping: 24, stiffness: 100, mass: 0.7 },
  });
  const exitOpacity = Math.max(0, 1 - Math.max(0, exit - 1));

  // Counter rolling up to 32
  const counterValue = Math.floor(interpolate(frame, [10, 70], [0, 32], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }));

  // Stagger grid reveal
  const gridEnter = (i: number) => {
    const col = i % 4;
    const row = Math.floor(i / 4);
    const delay = 25 + (col + row) * 2;
    return spring({
      frame: frame - delay,
      fps,
      config: { damping: 18, stiffness: 120, mass: 0.5 },
    });
  };

  return (
    <AbsoluteFill className="themes-scene" style={{ opacity: exitOpacity }}>
      <Background fadeIn={0} />
      <AbsoluteFill style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between", padding: "0 100px" }}>
        <FeatureCard
          feature={{
            id: "themes",
            kicker: "32 themes",
            title: "Ten new lights + Claude Code",
            body: "Brand light counterparts, custom cool designs, and a warm-charcoal Claude Code look. Each ships a complete token set.",
            accent: "cyan",
          }}
          enterAt={FEATURE_ENTER}
          durationInFrames={DURATION}
          side="left"
        />

        <div style={{ display: "flex", flexDirection: "column", alignItems: "flex-end", gap: 24 }}>
          {/* Counter */}
          <div style={{ display: "flex", alignItems: "baseline", gap: 12 }}>
            <div
              style={{
                fontFamily: "var(--font-sans)",
                fontSize: 140,
                fontWeight: 800,
                lineHeight: 1,
                color: "var(--text-bright)",
                fontFeatureSettings: '"tnum"',
                letterSpacing: "-0.04em",
              }}
            >
              {String(counterValue).padStart(2, "0")}
            </div>
            <div
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: 22,
                color: "var(--text-muted)",
                letterSpacing: "0.08em",
                textTransform: "uppercase",
              }}
            >
              themes
            </div>
          </div>

          {/* Grid */}
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(4, 1fr)",
              gap: 14,
            }}
          >
            {THEMES.map((theme, i) => {
              const enter = gridEnter(i);
              return (
                <div
                  key={theme.id}
                  style={{
                    width: 120,
                    height: 80,
                    borderRadius: 12,
                    background: theme.bg,
                    border: `1px solid ${theme.accent}40`,
                    padding: 10,
                    display: "flex",
                    flexDirection: "column",
                    justifyContent: "space-between",
                    boxShadow: `0 8px 20px -4px ${theme.accent}22`,
                    opacity: enter,
                    transform: `scale(${0.7 + 0.3 * enter})`,
                  }}
                >
                  <div
                    style={{
                      fontFamily: "var(--font-sans)",
                      fontSize: 12,
                      fontWeight: 600,
                      color: theme.text,
                      whiteSpace: "nowrap",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                    }}
                  >
                    {theme.name}
                  </div>
                  <div
                    style={{
                      width: 18,
                      height: 18,
                      borderRadius: 6,
                      background: theme.accent,
                      alignSelf: "flex-end",
                      boxShadow: `0 0 8px ${theme.accent}80`,
                    }}
                  />
                </div>
              );
            })}
          </div>
        </div>
      </AbsoluteFill>
      <style>{`
        .themes-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

export const THEMES_DURATION = DURATION;
