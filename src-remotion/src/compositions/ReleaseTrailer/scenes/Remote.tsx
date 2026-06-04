import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background, FeatureCard } from "../components";

const DURATION = 120;
const FEATURE_ENTER = 12;

type Connection = {
  name: string;
  host: string;
  auth: "password" | "key" | "agent";
  resume: "tmux" | "keepalive";
  defaultDir: string;
  delay: number;
  selected?: boolean;
};

const CONNECTIONS: Connection[] = [
  {
    name: "build-server",
    host: "root@10.0.1.5:22",
    auth: "key",
    resume: "tmux",
    defaultDir: "/var/lib/blxcode",
    delay: 18,
    selected: true,
  },
  {
    name: "staging-1",
    host: "ops@stg-1.example.com:2222",
    auth: "agent",
    resume: "keepalive",
    defaultDir: "~/work/staging",
    delay: 24,
  },
  {
    name: "edge-fra-1",
    host: "deploy@fra-1.example.com:22",
    auth: "password",
    resume: "tmux",
    defaultDir: "~/services",
    delay: 30,
  },
  {
    name: "macbook-air",
    host: "iptoux@mac.local:22",
    auth: "key",
    resume: "keepalive",
    defaultDir: "~/code",
    delay: 36,
  },
];

const authLabel = (a: Connection["auth"]) =>
  a === "password" ? "Password" : a === "key" ? "Key file" : "SSH agent";

const authIcon = (a: Connection["auth"]) => (a === "password" ? "🔑" : a === "key" ? "🗝" : "⚙");

const resumeLabel = (r: Connection["resume"]) => (r === "tmux" ? "tmux" : "keepalive");

export const RemoteScene: React.FC = () => {
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

  // editor view fade in
  const editorOpen = interpolate(frame, [40, 55], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const editorX = interpolate(frame, [40, 55], [40, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  return (
    <AbsoluteFill className="remote-scene" style={{ opacity: exitOpacity }}>
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
            id: "remote",
            kicker: "Remote",
            title: "SSH workspaces, master/detail settings",
            body: "tmux-persistent or keepalive sessions, password / key / agent auth, OS-keychain secret storage, and a grid-of-cards settings view.",
            accent: "blue",
          }}
          enterAt={FEATURE_ENTER}
          durationInFrames={DURATION}
          side="left"
        />

        <div
          style={{
            width: 740,
            opacity: cardEnter,
            transform: `translateY(${(1 - cardEnter) * 20}px)`,
            position: "relative",
          }}
        >
          {/* Cards grid */}
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(2, 1fr)",
              gap: 14,
            }}
          >
            {CONNECTIONS.map((c) => {
              const enter = spring({
                frame: frame - c.delay,
                fps,
                config: { damping: 16, stiffness: 130, mass: 0.5 },
              });
              return (
                <div
                  key={c.name}
                  style={{
                    padding: 16,
                    background: c.selected ? "var(--bg-panel)" : "var(--bg-raised)",
                    border: `1px solid ${c.selected ? "var(--accent-blue)" : "var(--border)"}`,
                    borderRadius: 14,
                    boxShadow: c.selected
                      ? "0 20px 50px -10px rgba(122, 162, 247, 0.3)"
                      : "none",
                    opacity: enter,
                    transform: `scale(${0.94 + 0.06 * enter}) translateY(${(1 - enter) * 12}px)`,
                  }}
                >
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 10,
                      marginBottom: 10,
                    }}
                  >
                    <span style={{ fontSize: 16, color: "var(--text-muted)" }}>▣</span>
                    <span
                      style={{
                        fontFamily: "var(--font-sans)",
                        fontSize: 17,
                        fontWeight: 700,
                        color: "var(--text-bright)",
                        flex: 1,
                      }}
                    >
                      {c.name}
                    </span>
                    <span
                      style={{
                        padding: "2px 8px",
                        borderRadius: 999,
                        background: "var(--accent-blue-soft)",
                        fontFamily: "var(--font-mono)",
                        fontSize: 10,
                        color: "var(--accent-blue)",
                        textTransform: "uppercase",
                        letterSpacing: "0.1em",
                      }}
                    >
                      SSH
                    </span>
                  </div>
                  <div
                    style={{
                      fontFamily: "var(--font-mono)",
                      fontSize: 12,
                      color: "var(--text-muted)",
                      marginBottom: 12,
                    }}
                  >
                    {c.host}
                  </div>
                  <div
                    style={{
                      display: "flex",
                      flexDirection: "column",
                      gap: 6,
                      fontFamily: "var(--font-mono)",
                      fontSize: 11,
                      color: "var(--text-faint)",
                    }}
                  >
                    <div style={{ display: "flex", gap: 6 }}>
                      <span>{authIcon(c.auth)}</span>
                      <span style={{ color: "var(--text-muted)" }}>{authLabel(c.auth)}</span>
                    </div>
                    <div style={{ display: "flex", gap: 6 }}>
                      <span>↻</span>
                      <span style={{ color: "var(--text-muted)" }}>
                        {resumeLabel(c.resume)}
                      </span>
                    </div>
                    <div style={{ display: "flex", gap: 6 }}>
                      <span>📁</span>
                      <span style={{ color: "var(--text-muted)" }}>{c.defaultDir}</span>
                    </div>
                  </div>
                  <div
                    style={{
                      marginTop: 10,
                      paddingTop: 10,
                      borderTop: "1px solid var(--border)",
                      display: "flex",
                      alignItems: "center",
                      gap: 6,
                      fontFamily: "var(--font-mono)",
                      fontSize: 10,
                      color: "var(--status-success)",
                      letterSpacing: "0.1em",
                      textTransform: "uppercase",
                    }}
                  >
                    <span
                      style={{
                        width: 6,
                        height: 6,
                        borderRadius: "50%",
                        background: "var(--status-success)",
                        boxShadow: "0 0 6px var(--status-success)",
                      }}
                    />
                    stored · OS keychain
                  </div>
                </div>
              );
            })}
          </div>

          {/* Editor overlay */}
          <div
            style={{
              position: "absolute",
              top: 0,
              right: 0,
              width: 360,
              height: "100%",
              opacity: editorOpen,
              transform: `translateX(${editorX}px)`,
              pointerEvents: "none",
            }}
          >
            <div
              style={{
                position: "relative",
                height: "100%",
                background: "var(--bg-panel)",
                border: "1px solid var(--accent-blue)",
                borderRadius: 14,
                padding: 16,
                boxShadow: "0 30px 80px -20px rgba(122, 162, 247, 0.4)",
                display: "flex",
                flexDirection: "column",
                gap: 12,
              }}
            >
              <div
                style={{
                  fontFamily: "var(--font-mono)",
                  fontSize: 11,
                  color: "var(--accent-blue)",
                  letterSpacing: "0.14em",
                  textTransform: "uppercase",
                }}
              >
                ← edit connection
              </div>
              <div
                style={{
                  fontFamily: "var(--font-sans)",
                  fontSize: 22,
                  fontWeight: 700,
                  color: "var(--text-bright)",
                }}
              >
                build-server
              </div>
              <Field label="host" value="10.0.1.5:22" />
              <Field label="user" value="root" />
              <Field label="auth" value="key file" />
              <Field label="session" value="tmux (persistent)" />
              <Field label="default dir" value="/var/lib/blxcode" />
              <div style={{ flex: 1 }} />
              <div
                style={{
                  display: "flex",
                  gap: 8,
                }}
              >
                <div
                  style={{
                    flex: 1,
                    padding: "10px 12px",
                    borderRadius: 8,
                    background: "var(--status-danger)",
                    color: "#16161e",
                    fontFamily: "var(--font-sans)",
                    fontSize: 14,
                    fontWeight: 600,
                    textAlign: "center",
                  }}
                >
                  Delete
                </div>
                <div
                  style={{
                    flex: 1,
                    padding: "10px 12px",
                    borderRadius: 8,
                    background: "var(--accent-blue)",
                    color: "#16161e",
                    fontFamily: "var(--font-sans)",
                    fontSize: 14,
                    fontWeight: 600,
                    textAlign: "center",
                  }}
                >
                  Save
                </div>
              </div>
            </div>
          </div>
        </div>
      </AbsoluteFill>
      <style>{`
        .remote-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

const Field: React.FC<{ label: string; value: string }> = ({ label, value }) => (
  <div
    style={{
      display: "flex",
      alignItems: "center",
      gap: 10,
      padding: "8px 10px",
      background: "var(--bg-raised)",
      border: "1px solid var(--border)",
      borderRadius: 8,
    }}
  >
    <span
      style={{
        fontFamily: "var(--font-mono)",
        fontSize: 10,
        color: "var(--text-faint)",
        letterSpacing: "0.12em",
        textTransform: "uppercase",
        width: 80,
      }}
    >
      {label}
    </span>
    <span
      style={{
        fontFamily: "var(--font-mono)",
        fontSize: 13,
        color: "var(--text-bright)",
        flex: 1,
      }}
    >
      {value}
    </span>
  </div>
);

export const REMOTE_DURATION = DURATION;
