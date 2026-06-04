import { AbsoluteFill, interpolate, useCurrentFrame, useVideoConfig } from "remotion";

export type BackgroundProps = {
  /** Frame range to fade in / out */
  fadeIn?: number;
  fadeOut?: number;
  /** Disable animated orbs for static shots */
  staticOrbs?: boolean;
};

const ORBS: { color: string; x: number; y: number; size: number; phase: number }[] = [
  { color: "#bd93f9", x: 18, y: 22, size: 520, phase: 0 },
  { color: "#7dcfff", x: 82, y: 30, size: 460, phase: 90 },
  { color: "#ff79c6", x: 28, y: 78, size: 480, phase: 180 },
  { color: "#7aa2f7", x: 78, y: 72, size: 420, phase: 270 },
];

export const Background: React.FC<BackgroundProps> = ({ fadeIn = 0, fadeOut, staticOrbs }) => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const inOpacity = interpolate(frame, [fadeIn, fadeIn + 18], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const outOpacity =
    fadeOut !== undefined
      ? interpolate(frame, [fadeOut, durationInFrames], [1, 0], {
          extrapolateLeft: "clamp",
          extrapolateRight: "clamp",
        })
      : 1;
  const opacity = inOpacity * outOpacity;

  return (
    <AbsoluteFill style={{ opacity, background: "var(--bg-app)" }}>
      <div className="bg-mesh" aria-hidden>
        <div className="bg-mesh__base" />
        {!staticOrbs &&
          ORBS.map((orb, i) => {
            const t = (frame + orb.phase) / 60;
            const dx = Math.sin(t) * 4;
            const dy = Math.cos(t * 0.8) * 3;
            return (
              <div
                key={i}
                className="bg-mesh__orb"
                style={{
                  left: `${orb.x}%`,
                  top: `${orb.y}%`,
                  width: orb.size,
                  height: orb.size,
                  background: `radial-gradient(circle, ${orb.color}55 0%, ${orb.color}00 65%)`,
                  transform: `translate(-50%, -50%) translate(${dx}%, ${dy}%)`,
                }}
              />
            );
          })}
        <div className="bg-mesh__grid" />
        <div className="bg-mesh__vignette" />
      </div>
      <style>{`
        .bg-mesh {
          position: absolute;
          inset: 0;
          overflow: hidden;
        }
        .bg-mesh__base {
          position: absolute;
          inset: 0;
          background:
            radial-gradient(ellipse 80% 60% at 50% 0%, rgba(189, 147, 249, 0.08), transparent 60%),
            radial-gradient(ellipse 60% 50% at 100% 100%, rgba(125, 207, 255, 0.06), transparent 60%),
            var(--bg-app);
        }
        .bg-mesh__orb {
          position: absolute;
          border-radius: 50%;
          filter: blur(40px);
          will-change: transform;
        }
        .bg-mesh__grid {
          position: absolute;
          inset: 0;
          background-image:
            linear-gradient(rgba(189, 200, 245, 0.025) 1px, transparent 1px),
            linear-gradient(90deg, rgba(189, 200, 245, 0.025) 1px, transparent 1px);
          background-size: 64px 64px;
          mask-image: radial-gradient(ellipse 60% 80% at 50% 50%, black 30%, transparent 80%);
        }
        .bg-mesh__vignette {
          position: absolute;
          inset: 0;
          background: radial-gradient(ellipse 100% 80% at 50% 50%, transparent 50%, rgba(0, 0, 0, 0.5) 100%);
        }
      `}</style>
    </AbsoluteFill>
  );
};
