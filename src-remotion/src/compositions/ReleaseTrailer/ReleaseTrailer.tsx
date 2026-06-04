import { AbsoluteFill, Sequence } from "remotion";
import { INTRO_DURATION, IntroScene } from "./scenes/Intro";
import { TITLEBAR_DURATION, TitlebarScene } from "./scenes/Titlebar";
import { THEME_DURATION, ThemeScene } from "./scenes/Theme";
import { ORB_DURATION, OrbScene } from "./scenes/Orb";
import { COMPOSER_DURATION, ComposerScene } from "./scenes/Composer";
import { CLI_AGENTS_DURATION, CliAgentsScene } from "./scenes/CliAgents";
import { TERMINALS_DURATION, TerminalsScene } from "./scenes/Terminals";
import { PLANS_DURATION, PlansScene } from "./scenes/Plans";
import { CHANGED_FILES_DURATION, ChangedFilesScene } from "./scenes/ChangedFiles";
import { STATS_DURATION, StatsScene } from "./scenes/Stats";
import { TOOL_LOOP_DURATION, ToolLoopScene } from "./scenes/ToolLoop";
import { FILES_DURATION, FilesScene } from "./scenes/Files";
import { MEMORY_DURATION, MemoryScene } from "./scenes/Memory";
import { FILTER_DURATION, FilterScene } from "./scenes/Filter";
import { GITGRAPH_DURATION, GitGraphScene } from "./scenes/GitGraph";
import { REMOTE_DURATION, RemoteScene } from "./scenes/Remote";
import { ROUNDINGS_DURATION, RoundingsScene } from "./scenes/Roundings";
import { THEMES_DURATION, ThemesScene } from "./scenes/Themes";
import { OUTRO_DURATION, OutroScene } from "./scenes/Outro";
import "./ReleaseTrailer.css";

const SCENES: { id: string; duration: number; component: React.FC }[] = [
  { id: "intro", duration: INTRO_DURATION, component: IntroScene },
  { id: "titlebar", duration: TITLEBAR_DURATION, component: TitlebarScene },
  { id: "theme", duration: THEME_DURATION, component: ThemeScene },
  { id: "orb", duration: ORB_DURATION, component: OrbScene },
  { id: "composer", duration: COMPOSER_DURATION, component: ComposerScene },
  { id: "cli-agents", duration: CLI_AGENTS_DURATION, component: CliAgentsScene },
  { id: "terminals", duration: TERMINALS_DURATION, component: TerminalsScene },
  { id: "plans", duration: PLANS_DURATION, component: PlansScene },
  { id: "changed-files", duration: CHANGED_FILES_DURATION, component: ChangedFilesScene },
  { id: "stats", duration: STATS_DURATION, component: StatsScene },
  { id: "tool-loop", duration: TOOL_LOOP_DURATION, component: ToolLoopScene },
  { id: "files", duration: FILES_DURATION, component: FilesScene },
  { id: "memory", duration: MEMORY_DURATION, component: MemoryScene },
  { id: "filter", duration: FILTER_DURATION, component: FilterScene },
  { id: "git", duration: GITGRAPH_DURATION, component: GitGraphScene },
  { id: "remote", duration: REMOTE_DURATION, component: RemoteScene },
  { id: "roundings", duration: ROUNDINGS_DURATION, component: RoundingsScene },
  { id: "themes", duration: THEMES_DURATION, component: ThemesScene },
  { id: "outro", duration: OUTRO_DURATION, component: OutroScene },
];

export const ReleaseTrailer: React.FC = () => {
  let offset = 0;
  return (
    <AbsoluteFill className="trailer">
      {SCENES.map((scene) => {
        const from = offset;
        offset += scene.duration;
        return (
          <Sequence key={scene.id} from={from} durationInFrames={scene.duration}>
            <scene.component />
          </Sequence>
        );
      })}
    </AbsoluteFill>
  );
};

export const RELEASE_TRAILER_DURATION = SCENES.reduce((acc, s) => acc + s.duration, 0);
