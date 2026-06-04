import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background } from "../../ReleaseTrailer/components/Background";

const DURATION = 150;

const WINDOW_W = 1100;
const WINDOW_H = 640;
const COLS = 5;
const ROWS = 3;
const TILE_W = WINDOW_W / COLS;
const TILE_H = WINDOW_H / ROWS;

type Shard = {
  col: number;
  row: number;
  dx: number;
  dy: number;
  rot: number;
  delay: number;
  scaleEnd: number;
};

const SHARDS: Shard[] = [
  { col: 0, row: 0, dx: -640, dy: -460, rot: -42, delay: 0, scaleEnd: 0.7 },
  { col: 1, row: 0, dx: -340, dy: -520, rot: -28, delay: 2, scaleEnd: 0.75 },
  { col: 2, row: 0, dx: -40, dy: -560, rot: -10, delay: 1, scaleEnd: 0.7 },
  { col: 3, row: 0, dx: 320, dy: -500, rot: 24, delay: 3, scaleEnd: 0.78 },
  { col: 4, row: 0, dx: 620, dy: -440, rot: 38, delay: 1, scaleEnd: 0.7 },
  { col: 0, row: 1, dx: -720, dy: -40, rot: -36, delay: 2, scaleEnd: 0.75 },
  { col: 1, row: 1, dx: -420, dy: -120, rot: -18, delay: 0, scaleEnd: 0.72 },
  { col: 2, row: 1, dx: 0, dy: -180, rot: 6, delay: 4, scaleEnd: 0.7 },
  { col: 3, row: 1, dx: 400, dy: -100, rot: 22, delay: 1, scaleEnd: 0.74 },
  { col: 4, row: 1, dx: 700, dy: -20, rot: 34, delay: 3, scaleEnd: 0.7 },
  { col: 0, row: 2, dx: -600, dy: 380, rot: -32, delay: 1, scaleEnd: 0.75 },
  { col: 1, row: 2, dx: -300, dy: 460, rot: -14, delay: 3, scaleEnd: 0.72 },
  { col: 2, row: 2, dx: 20, dy: 500, rot: 8, delay: 0, scaleEnd: 0.7 },
  { col: 3, row: 2, dx: 320, dy: 440, rot: 18, delay: 2, scaleEnd: 0.74 },
  { col: 4, row: 2, dx: 640, dy: 360, rot: 30, delay: 1, scaleEnd: 0.7 },
];

const OldMockup: React.FC = () => (
  <div
    style={{
      width: WINDOW_W,
      height: WINDOW_H,
      background: "#1c1c24",
      border: "1px solid #2a2a35",
      borderRadius: 12,
      overflow: "hidden",
      boxShadow: "0 50px 120px -30px rgba(0, 0, 0, 0.8)",
      filter: "saturate(0.45) brightness(0.78) contrast(0.95)",
      fontFamily: "var(--font-sans)",
    }}
  >
    <div
      style={{
        display: "flex",
        alignItems: "center",
        height: 40,
        padding: "0 14px",
        background: "#23232c",
        borderBottom: "1px solid #2a2a35",
        gap: 10,
      }}
    >
      <div style={{ display: "flex", gap: 6 }}>
        <div style={{ width: 11, height: 11, borderRadius: "50%", background: "#5a5a66" }} />
        <div style={{ width: 11, height: 11, borderRadius: "50%", background: "#5a5a66" }} />
        <div style={{ width: 11, height: 11, borderRadius: "50%", background: "#5a5a66" }} />
      </div>
      <div
        style={{
          flex: 1,
          textAlign: "center",
          fontSize: 13,
          color: "#5a5a66",
          fontFamily: "var(--font-mono)",
          letterSpacing: "0.04em",
        }}
      >
        blxcode — v0.3.3
      </div>
      <div style={{ width: 50 }} />
    </div>

    <div style={{ display: "flex", height: WINDOW_H - 40 }}>
      <div
        style={{
          width: 200,
          background: "#1a1a22",
          borderRight: "1px solid #2a2a35",
          padding: "16px 0",
        }}
      >
        {["Workspace", "Files", "Terminal", "Settings", "Help"].map((item, i) => (
          <div
            key={item}
            style={{
              padding: "9px 18px",
              fontSize: 13,
              color: i === 0 ? "#6e6e80" : "#4a4a55",
              background: i === 0 ? "#23232c" : "transparent",
              borderLeft: i === 0 ? "2px solid #4a4a55" : "2px solid transparent",
            }}
          >
            {item}
          </div>
        ))}
      </div>

      <div style={{ flex: 1, padding: 22, display: "flex", flexDirection: "column", gap: 14 }}>
        <div style={{ fontSize: 12, color: "#5a5a66", fontFamily: "var(--font-mono)" }}>
          ~/blxcode/main.py
        </div>
        <div
          style={{
            flex: 1,
            background: "#16161c",
            border: "1px solid #23232c",
            borderRadius: 6,
            padding: 16,
            fontFamily: "var(--font-mono)",
            fontSize: 13,
            color: "#4a4a55",
            lineHeight: 1.6,
          }}
        >
          <div>def main():</div>
          <div>&nbsp;&nbsp;run_agent_loop()</div>
          <div>&nbsp;&nbsp;return result</div>
          <div>&nbsp;</div>
          <div>if __name__ == &quot;__main__&quot;:</div>
          <div>&nbsp;&nbsp;main()</div>
        </div>
        <div
          style={{
            display: "flex",
            gap: 8,
            padding: "8px 12px",
            background: "#1a1a22",
            border: "1px solid #23232c",
            borderRadius: 6,
            fontFamily: "var(--font-mono)",
            fontSize: 12,
            color: "#4a4a55",
          }}
        >
          <span>$</span>
          <span>ready</span>
        </div>
      </div>
    </div>
  </div>
);

const NewMockup: React.FC = () => (
  <div
    style={{
      width: WINDOW_W,
      height: WINDOW_H,
      background: "var(--bg-panel)",
      border: "1px solid var(--border-strong)",
      borderRadius: 16,
      overflow: "hidden",
      boxShadow:
        "0 60px 140px -30px rgba(0, 0, 0, 0.8), 0 0 0 1px rgba(189, 200, 245, 0.04) inset, 0 0 80px -10px rgba(189, 147, 249, 0.25)",
      fontFamily: "var(--font-sans)",
    }}
  >
    <div
      style={{
        display: "flex",
        alignItems: "center",
        height: 52,
        padding: "0 16px",
        background: "var(--bg-panel-header)",
        borderBottom: "1px solid var(--border)",
        gap: 14,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 10, width: 180 }}>
        <div
          style={{
            width: 26,
            height: 26,
            borderRadius: 8,
            background: "linear-gradient(135deg, var(--accent-purple) 0%, var(--accent-pink) 100%)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 16,
            fontWeight: 800,
            color: "#16161e",
          }}
        >
          b
        </div>
        <div style={{ fontSize: 15, fontWeight: 700, color: "var(--text-bright)" }}>blxcode</div>
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 6, width: 70 }}>
        <div
          style={{
            width: 30,
            height: 30,
            borderRadius: 6,
            background: "var(--bg-raised)",
            border: "1px solid var(--border)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 12,
            color: "var(--text-muted)",
          }}
        >
          ◧
        </div>
      </div>

      <div
        style={{
          flex: 1,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          gap: 8,
        }}
      >
        {[
          { name: "blxcode", color: "var(--accent-purple)" },
          { name: "src", color: "var(--accent-cyan)" },
          { name: "workbench", color: "var(--accent-pink)" },
          { name: "titlebar", color: "var(--status-success)" },
        ].map((b, i, arr) => (
          <div key={b.name} style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <span
              style={{
                width: 7,
                height: 7,
                borderRadius: "50%",
                background: b.color,
                boxShadow: `0 0 8px ${b.color}`,
              }}
            />
            <span
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: 13,
                color: i === arr.length - 1 ? "var(--text-bright)" : "var(--text-muted)",
                fontWeight: i === arr.length - 1 ? 600 : 400,
              }}
            >
              {b.name}
            </span>
            {i < arr.length - 1 && <span style={{ color: "var(--text-faint)", fontSize: 12 }}>›</span>}
          </div>
        ))}
      </div>

      <div
        style={{
          padding: "6px 12px",
          borderRadius: 6,
          background: "var(--accent-purple)",
          color: "#16161e",
          fontFamily: "var(--font-mono)",
          fontSize: 11,
          fontWeight: 600,
          letterSpacing: "0.1em",
        }}
      >
        NAVIGATE
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 6, marginLeft: 6 }}>
        <div
          style={{
            width: 30,
            height: 30,
            borderRadius: 6,
            background: "transparent",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 12,
            color: "var(--text-muted)",
          }}
        >
          —
        </div>
        <div
          style={{
            width: 30,
            height: 30,
            borderRadius: 6,
            background: "transparent",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 12,
            color: "var(--text-muted)",
          }}
        >
          ◻
        </div>
        <div
          style={{
            width: 30,
            height: 30,
            borderRadius: 6,
            background: "transparent",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 12,
            color: "var(--text-muted)",
          }}
        >
          ✕
        </div>
      </div>
    </div>

    <div style={{ display: "flex", height: WINDOW_H - 52 }}>
      <div
        style={{
          width: 200,
          background: "var(--bg-raised)",
          borderRight: "1px solid var(--border)",
          padding: "16px 0",
        }}
      >
        {[
          { name: "Workspace", accent: "var(--accent-purple)" },
          { name: "Files", accent: "var(--accent-cyan)" },
          { name: "Terminal", accent: "var(--accent-pink)" },
          { name: "Plans", accent: "var(--status-success)" },
          { name: "Memory", accent: "var(--status-warning)" },
          { name: "Settings", accent: "var(--accent-blue)" },
        ].map((item, i) => (
          <div
            key={item.name}
            style={{
              padding: "10px 18px",
              fontSize: 13,
              color: i === 0 ? "var(--text-bright)" : "var(--text-muted)",
              background: i === 0 ? "var(--bg-panel)" : "transparent",
              borderLeft: i === 0 ? `2px solid ${item.accent}` : "2px solid transparent",
              fontWeight: i === 0 ? 600 : 400,
            }}
          >
            {item.name}
          </div>
        ))}
      </div>

      <div style={{ flex: 1, padding: 22, display: "flex", flexDirection: "column", gap: 14 }}>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 10,
            padding: "6px 12px",
            borderRadius: 999,
            background: "var(--bg-raised)",
            border: "1px solid var(--border)",
            width: "fit-content",
            fontFamily: "var(--font-mono)",
            fontSize: 12,
            color: "var(--text-muted)",
          }}
        >
          <span
            style={{
              width: 7,
              height: 7,
              borderRadius: "50%",
              background: "var(--status-success)",
              boxShadow: "0 0 6px var(--status-success)",
            }}
          />
          claude · opus 4.6
        </div>
        <div
          style={{
            flex: 1,
            background: "var(--bg-raised)",
            border: "1px solid var(--border)",
            borderRadius: 10,
            padding: 16,
            fontFamily: "var(--font-sans)",
            fontSize: 16,
            color: "var(--text-bright)",
            lineHeight: 1.5,
          }}
        >
          <span style={{ color: "var(--text-faint)" }}>› </span>
          add a /health endpoint that returns the orb mode and last turn duration
          <span
            style={{
              display: "inline-block",
              width: 2,
              height: 18,
              background: "var(--accent-pink)",
              marginLeft: 2,
              verticalAlign: "text-bottom",
            }}
          />
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <div
            style={{
              padding: "8px 14px",
              borderRadius: 10,
              background: "var(--bg-raised)",
              border: "1px solid var(--border)",
              fontSize: 13,
              color: "var(--text)",
            }}
          >
            Plan
          </div>
          <div
            style={{
              padding: "8px 14px",
              borderRadius: 10,
              background: "var(--bg-raised)",
              border: "1px solid var(--border)",
              fontSize: 13,
              color: "var(--text)",
            }}
          >
            Standard
          </div>
          <div style={{ flex: 1 }} />
          <div
            style={{
              width: 36,
              height: 36,
              borderRadius: 12,
              background: "linear-gradient(135deg, var(--accent-pink) 0%, var(--accent-purple) 100%)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: "#16161e",
              fontWeight: 700,
            }}
          >
            ↑
          </div>
        </div>
      </div>
    </div>
  </div>
);

const Shard: React.FC<{ shard: Shard; opacity: number; transform: string }> = ({
  shard,
  opacity,
  transform,
}) => (
  <div
    style={{
      position: "absolute",
      left: 0,
      top: 0,
      width: TILE_W,
      height: TILE_H,
      overflow: "hidden",
      pointerEvents: "none",
      opacity,
      transform,
      transformOrigin: "center center",
      willChange: "transform, opacity",
    }}
  >
    <div
      style={{
        position: "absolute",
        left: -shard.col * TILE_W,
        top: -shard.row * TILE_H,
        width: WINDOW_W,
        height: WINDOW_H,
      }}
    >
      <OldMockup />
    </div>
  </div>
);

const ShardGrid: React.FC<{ progress: number; visible: boolean }> = ({ progress, visible }) => {
  if (!visible) return null;
  return (
    <>
      {SHARDS.map((shard, i) => {
        const p = Math.max(0, Math.min(1, (progress - shard.delay * 0.01) / 0.5));
        const eased = p * p * (3 - 2 * p);
        const dx = shard.dx * eased;
        const dy = shard.dy * eased;
        const rot = shard.rot * eased;
        const scale = 1 - (1 - shard.scaleEnd) * eased;
        const opacity = 1 - eased;
        return (
          <Shard
            key={i}
            shard={shard}
            opacity={opacity}
            transform={`translate(${dx}px, ${dy}px) rotate(${rot}deg) scale(${scale})`}
          />
        );
      })}
    </>
  );
};

export const StartScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const enter = spring({
    frame: frame - 2,
    fps,
    config: { damping: 18, stiffness: 90, mass: 0.8 },
  });

  const IMPACT_FRAME = 56;
  const SHATTER_END = 92;

  const oldUiProgress = interpolate(frame, [10, 28], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const oldUiEnter = spring({
    frame: frame - 10,
    fps,
    config: { damping: 18, stiffness: 90, mass: 0.7 },
  });

  const glitch = frame >= IMPACT_FRAME - 6 && frame < IMPACT_FRAME;
  const glitchAmount = glitch ? (1 - (frame - (IMPACT_FRAME - 6)) / 6) : 0;

  const shatterProgress = interpolate(frame, [IMPACT_FRAME, SHATTER_END], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const flashOpacity = interpolate(frame, [IMPACT_FRAME, IMPACT_FRAME + 4, IMPACT_FRAME + 12], [0, 1, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const flashScale = interpolate(frame, [IMPACT_FRAME, IMPACT_FRAME + 12], [0.4, 2.2], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const newUiEnter = spring({
    frame: frame - (IMPACT_FRAME + 6),
    fps,
    config: { damping: 16, stiffness: 100, mass: 0.7 },
  });
  const newUiOpacity = interpolate(
    frame,
    [IMPACT_FRAME + 6, IMPACT_FRAME + 26],
    [0, 1],
    { extrapolateLeft: "clamp", extrapolateRight: "clamp" },
  );
  const showNewUi = frame >= IMPACT_FRAME + 4;

  const oldUiFade = interpolate(frame, [IMPACT_FRAME - 2, IMPACT_FRAME + 2], [1, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const showOldUi = frame < IMPACT_FRAME + 2;

  const kickerOpacity = interpolate(frame, [0, 12], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const bottomLabelOpacity = interpolate(frame, [10, 28], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const bottomLabelSwap = frame >= IMPACT_FRAME ? 1 : 0;

  const exit = spring({
    frame: frame - (DURATION - 14),
    fps,
    config: { damping: 24, stiffness: 100, mass: 0.7 },
  });
  const exitOpacity = Math.max(0, 1 - Math.max(0, exit - 1));

  return (
    <AbsoluteFill className="start-scene" style={{ opacity: exitOpacity }}>
      <Background fadeIn={0} />

      <div
        style={{
          position: "absolute",
          top: 60,
          left: 0,
          right: 0,
          display: "flex",
          justifyContent: "center",
          opacity: kickerOpacity,
          transform: `translateY(${(1 - kickerOpacity) * 8}px)`,
        }}
      >
        <div
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 12,
            fontFamily: "var(--font-mono)",
            fontSize: 18,
            fontWeight: 500,
            letterSpacing: "0.16em",
            textTransform: "uppercase",
            color: "var(--text-muted)",
          }}
        >
          <span
            style={{
              width: 8,
              height: 8,
              borderRadius: "50%",
              background: "var(--accent-purple)",
              boxShadow: "0 0 12px var(--accent-purple)",
            }}
          />
          blxcode — evolution
        </div>
      </div>

      <div
        style={{
          position: "absolute",
          inset: 0,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          opacity: oldUiProgress * enter,
        }}
      >
        <div
          style={{
            position: "relative",
            width: WINDOW_W,
            height: WINDOW_H,
            opacity: oldUiEnter,
            transform: `translateY(${(1 - oldUiEnter) * 16}px) scale(${0.96 + 0.04 * oldUiEnter})`,
          }}
        >
          {showOldUi && (
            <div
              style={{
                opacity: oldUiFade,
                transform: glitch
                  ? `translate(${(glitchAmount * 6 - 3).toFixed(1)}px, 0) skewX(${(glitchAmount * 1.4).toFixed(2)}deg)`
                  : "none",
                filter: glitch
                  ? `hue-rotate(${(glitchAmount * 20).toFixed(1)}deg) saturate(${0.45 + glitchAmount * 0.4})`
                  : "saturate(0.45) brightness(0.78) contrast(0.95)",
              }}
            >
              <OldMockup />
            </div>
          )}

          {frame >= IMPACT_FRAME - 1 && frame <= SHATTER_END + 2 && (
            <div
              style={{
                position: "absolute",
                left: 0,
                top: 0,
                width: WINDOW_W,
                height: WINDOW_H,
                pointerEvents: "none",
              }}
            >
              <ShardGrid progress={shatterProgress} visible />
            </div>
          )}

          {showNewUi && (
            <div
              style={{
                position: "absolute",
                left: 0,
                top: 0,
                opacity: newUiOpacity,
                transform: `scale(${0.94 + 0.06 * newUiEnter}) translateY(${(1 - newUiEnter) * 18}px)`,
              }}
            >
              <NewMockup />
            </div>
          )}

          {flashOpacity > 0 && (
            <div
              style={{
                position: "absolute",
                left: "50%",
                top: "50%",
                width: WINDOW_W,
                height: WINDOW_H,
                transform: `translate(-50%, -50%) scale(${flashScale})`,
                background:
                  "radial-gradient(circle, rgba(189, 147, 249, 0.85) 0%, rgba(125, 207, 255, 0.5) 35%, transparent 70%)",
                filter: "blur(30px)",
                opacity: flashOpacity,
                pointerEvents: "none",
                mixBlendMode: "screen",
              }}
            />
          )}
        </div>
      </div>

      <div
        style={{
          position: "absolute",
          bottom: 70,
          left: 0,
          right: 0,
          display: "flex",
          justifyContent: "center",
          opacity: bottomLabelOpacity,
        }}
      >
        <div
          style={{
            position: "relative",
            display: "inline-flex",
            alignItems: "center",
            gap: 14,
            padding: "10px 20px",
            borderRadius: 999,
            border: "1px solid var(--border-strong)",
            background: "rgba(22, 22, 30, 0.7)",
            fontFamily: "var(--font-mono)",
            fontSize: 18,
            color: "var(--text)",
            backdropFilter: "blur(8px)",
            minWidth: 280,
            justifyContent: "center",
          }}
        >
          <div
            style={{
              position: "absolute",
              inset: 0,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: 14,
              padding: "0 20px",
              opacity: 1 - bottomLabelSwap,
            }}
          >
            <span style={{ color: "var(--text-faint)" }}>v0.3.3</span>
            <span style={{ color: "var(--text-faint)" }}>→</span>
            <span style={{ color: "var(--text-muted)" }}>?????</span>
          </div>
          <div
            style={{
              position: "absolute",
              inset: 0,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: 14,
              padding: "0 20px",
              opacity: bottomLabelSwap,
            }}
          >
            <span style={{ color: "var(--text-muted)" }}>v0.3.3</span>
            <span style={{ color: "var(--accent-cyan)" }}>→</span>
            <span
              style={{
                color: "var(--accent-purple)",
                fontWeight: 700,
                textShadow: "0 0 16px rgba(189, 147, 249, 0.6)",
              }}
            >
              v0.3.4+
            </span>
          </div>
          <span style={{ visibility: "hidden" }}>v0.3.3 → v0.3.4+</span>
        </div>
      </div>

      <div
        style={{
          position: "absolute",
          inset: 0,
          pointerEvents: "none",
          opacity: 0.04,
          backgroundImage:
            "repeating-linear-gradient(0deg, rgba(255,255,255,0.6) 0px, rgba(255,255,255,0.6) 1px, transparent 1px, transparent 3px)",
          mixBlendMode: "overlay",
        }}
      />

      <style>{`
        .start-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

export const START_DURATION = DURATION;
