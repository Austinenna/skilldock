export type PaletteKey = 'cool' | 'warm' | 'slate';
export type Density = 'compact' | 'comfortable';
export type View = 'list' | 'grid';

/** See State in crates/skl. */
export type SkillState = 'devOnly' | 'released' | 'devAhead' | 'releasedOnly' | 'manifestOnly';

export interface Agent {
  id: string;
  path: string;
  installed: boolean;
}

export interface Skill {
  id: string;
  description: string;
  state: SkillState;
  agents: string[];
  hasDev: boolean;
  hasReleased: boolean;
}

export interface Overview {
  root: string;
  rootSource: string;
  manifestPath: string;
  agents: Agent[];
  skills: Skill[];
}

export interface SkillFile {
  name: string;
  kind: string;
  size: string;
  modified: string;
}

export interface SkillDetail {
  skillMd: string;
  devPath: string | null;
  releasedPath: string | null;
  files: SkillFile[];
}

export interface SyncLine {
  agent: string;
  name: string;
  action: 'created' | 'fixed' | 'removed' | 'conflict';
  message: string;
}

export interface SyncResult {
  changes: SyncLine[];
  kept: number;
  notes: string[];
  warnings: string[];
  git: string | null;
}

export interface Finding {
  level: 'error' | 'warn' | 'info';
  message: string;
}

export interface Config {
  palette: PaletteKey;
  density: Density;
  view: View;
}
