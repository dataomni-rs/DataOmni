import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import type { ConnectionProfile } from '../contracts/connection';
import { translateNow } from '../stores/languageStore';
import {
  parseConnectionImportFile,
  type ParsedConnectionImport,
} from './externalConnections';

/** 后端 `import_connections` / `import_connection_profiles` 的结果 */
export interface ConnectionImport {
  imported: number;
  /** 本机已经有的同一个连接，没再加一份 */
  skipped: number;
}

const EXPORT_FILTERS = [{ name: 'JSON', extensions: ['json'] }];
const IMPORT_FILTERS = [
  { name: 'Connections (DataOmni, Navicat .ncx, DBeaver)', extensions: ['json', 'ncx', 'xml'] },
];

/** 导入之后跟用户说的那句话。口令不在文件里，这件事每次都要说：不说的话第一次连接弹出口令框像是出了错 */
export function importSummary({ imported, skipped }: ConnectionImport): string {
  if (imported === 0) {
    return skipped === 0
      ? translateNow('connectionTransfer.empty')
      : translateNow('connectionTransfer.allPresent', { count: skipped });
  }
  const done =
    skipped === 0
      ? translateNow('connectionTransfer.imported', { count: imported })
      : translateNow('connectionTransfer.importedSkipped', { count: imported, skipped });
  return `${done}${translateNow('connectionTransfer.secretsNotIncluded')}`;
}

/** 选位置、写文件，返回要显示的那句话；取消了返回 `null`。出错照常抛，调用方按错误显示 */
export async function exportConnections(): Promise<string | null> {
  const path = await save({ defaultPath: 'dataomni-connections.json', filters: EXPORT_FILTERS });
  if (!path) {
    return null;
  }
  const count = await invoke<number>('export_connections', { path });
  return translateNow('connectionTransfer.exported', { count, path });
}

/** 选文件并解析出供预览确认的连接清单（支持 DataOmni JSON、Navicat `.ncx` 与 DBeaver `data-sources.json`） */
export async function pickConnectionImportFile(): Promise<ParsedConnectionImport | null> {
  const path = await open({ multiple: false, directory: false, filters: IMPORT_FILTERS });
  if (typeof path !== 'string') {
    return null;
  }
  const file = await invoke<{ contents: string }>('read_text_file', { path });
  return parseConnectionImportFile(file.contents);
}

/** 将预览对话框中勾选的连接写入本机配置并返回结果说明 */
export async function commitConnectionImport(profiles: ConnectionProfile[]): Promise<string> {
  const report = await invoke<ConnectionImport>('import_connection_profiles', { profiles });
  return importSummary(report);
}
