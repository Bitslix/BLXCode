import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background } from "../components/Background";

const DURATION = 150;

const ThemeSwatch: React.FC<{
  color: string;
  label: string;
  x: number;
  y: number;
  delay: number;
  size?: number;
  shape?: "circle" | "square";
}> = ({ color, label, x, y, delay, size = 90, shape = "circle" }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const enter = spring({
    frame: frame - delay,
    fps,
    config: { damping: 18, stiffness: 110, mass: 0.6 },
  });
  const enterScale = 0.4 + 0.6 * enter;
  const enterOpacity = Math.max(0, enter);

  return (
    <div
      style={{
        position: "absolute",
        left: x,
        top: y,
        opacity: enterOpacity,
        transform: `scale(${enterScale})`,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        gap: 10,
      }}
    >
      <div
        style={{
          width: size,
          height: size,
          borderRadius: shape === "circle" ? "50%" : 14,
          background: color,
          boxShadow: `0 0 40px ${color}80, 0 0 0 4px rgba(0, 0, 0, 0.2) inset`,
        }}
      />
      <div
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 16,
          fontWeight: 500,
          letterSpacing: "0.06em",
          color: "var(--text-muted)",
          textTransform: "uppercase",
        }}
      >
        {label}
      </div>
    </div>
  );
};

export const ThemeScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const titleEnter = spring({
    frame,
    fps,
    config: { damping: 16, stiffness: 80, mass: 0.8 },
  });
  const titleOpacity = interpolate(frame, [0, 18], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  const cardEnter = spring({
    frame: frame - 12,
    fps,
    config: { damping: 18, stiffness: 80, mass: 0.8 },
  });
  const cardOpacity = interpolate(frame, [12, 35], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  const arrowEnter = interpolate(frame, [50, 70], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const arrowX = interpolate(frame, [50, 75], [-30, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  const exit = spring({
    frame: frame - (DURATION - 18),
    fps,
    config: { damping: 24, stiffness: 100, mass: 0.7 },
  });
  const exitOpacity = Math.max(0, 1 - Math.max(0, exit - 1));

  return (
    <AbsoluteFill className="theme-scene" style={{ opacity: exitOpacity }}>
      <Background fadeIn={0} staticOrbs />
      <AbsoluteFill style={{ flexDirection: "row", alignItems: "center", justifyContent: "center", gap: 80, padding: "0 100px" }}>
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            gap: 24,
            opacity: titleOpacity,
            transform: `scale(${0.9 + 0.1 * titleEnter})`,
          }}
        >
          <div
            style={{
              fontFamily: "var(--font-mono)",
              fontSize: 20,
              fontWeight: 500,
              letterSpacing: "0.16em",
              textTransform: "uppercase",
              color: "var(--accent-purple)",
            }}
          >
            Tokyo Night × Dracula
          </div>
          <h2
            style={{
              fontSize: 110,
              fontWeight: 800,
              lineHeight: 0.95,
              letterSpacing: "-0.035em",
              margin: 0,
              color: "var(--text-bright)",
              textAlign: "center",
              maxWidth: 700,
            }}
          >
            a new look
          </h2>
        </div>

        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 60,
            opacity: cardOpacity,
            transform: `translateY(${(1 - cardEnter) * 30}px)`,
          }}
        >
          <div
            style={{
              width: 280,
              height: 360,
              borderRadius: 24,
              background: "#0a0a14",
              border: "2px solid var(--accent-blue)",
              padding: 28,
              display: "flex",
              flexDirection: "column",
              gap: 18,
              boxShadow: "0 30px 60px -20px rgba(0, 0, 0, 0.6)",
            }}
          >
            <div style={{ fontFamily: "var(--font-mono)", fontSize: 18, color: "#565f89", marginBottom: 8 }}>
              github
            </div>
            <div style={{ fontSize: 28, fontWeight: 600, color: "#f1f2f5" }}>GitHub blue</div>
            <div
              style={{
                marginTop: "auto",
                display: "flex",
                alignItems: "center",
                gap: 8,
                padding: "8px 16px",
                borderRadius: 999,
                background: "rgba(88, 166, 255, 0.12)",
                color: "#7ab8ff",
                fontFamily: "var(--font-mono)",
                fontSize: 18,
                width: "fit-content",
              }}
            >
              <span style={{ width: 8, height: 8, borderRadius: "50%", background: "#58a6ff" }} />
              legacy
            </div>
          </div>

          <div
            style={{
              opacity: arrowEnter,
              transform: `translateX(${arrowX}px)`,
              fontSize: 80,
              color: "var(--accent-purple)",
              fontWeight: 300,
            }}
          >
            →
          </div>

          <div
            style={{
              width: 280,
              height: 360,
              borderRadius: 24,
              background: "linear-gradient(180deg, #1f2030 0%, #16161e 100%)",
              border: "2px solid var(--accent-purple)",
              padding: 28,
              display: "flex",
              flexDirection: "column",
              gap: 18,
              boxShadow: "0 30px 60px -20px rgba(189, 147, 249, 0.4)",
            }}
          >
            <div style={{ fontFamily: "var(--font-mono)", fontSize: 18, color: "#a9b1d6", marginBottom: 8 }}>
              tokyo night
            </div>
            <div style={{ fontSize: 28, fontWeight: 600, color: "#f8f8f2" }}>Night blue</div>
            <div
              style={{
                marginTop: "auto",
                display: "flex",
                alignItems: "center",
                gap: 8,
                padding: "8px 16px",
                borderRadius: 999,
                background: "rgba(189, 147, 249, 0.18)",
                color: "#cda9fb",
                fontFamily: "var(--font-mono)",
                fontSize: 18,
                width: "fit-content",
              }}
            >
              <span style={{ width: 8, height: 8, borderRadius: "50%", background: "#bd93f9" }} />
              default
            </div>
          </div>
        </div>
      </AbsoluteFill>

      <ThemeSwatch color="#bd93f9" label="purple" x={120} y={850} delay={45} />
      <ThemeSwatch color="#7dcfff" label="cyan" x={250} y={850} delay={52} />
      <ThemeSwatch color="#ff79c6" label="pink" x={380} y={850} delay={59} shape="square" />
      <ThemeSwatch color="#50fa7b" label="green" x={510} y={850} delay={66} shape="square" />

      <ThemeSwatch color="#7aa2f7" label="blue" x={900} y={850} delay={73} />
      <ThemeSwatch color="#ffb86c" label="amber" x={1030} y={850} delay={80} shape="square" />
      <ThemeSwatch color="#ff5555" label="red" x={1160} y={850} delay={87} shape="square" />

      <style>{`
        .theme-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

export const THEME_DURATION = DURATION;
