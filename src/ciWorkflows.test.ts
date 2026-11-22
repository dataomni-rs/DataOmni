import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

type Scopes = {
  appChanged: boolean;
  siteChanged: boolean;
};

const NODE20_DEPRECATED_ACTIONS: ReadonlyArray<readonly [string, number]> = [
  ['actions/checkout', 7],
  ['actions/cache', 6],
  ['actions/upload-artifact', 7],
  ['actions/download-artifact', 8],
  ['actions/upload-pages-artifact', 5],
  ['actions/deploy-pages', 5],
];

function findOutdatedNode20Actions(workflowName: string, yamlText: string): string[] {
  const outdated: string[] = [];
  for (const [action, minMajor] of NODE20_DEPRECATED_ACTIONS) {
    const escaped = action.replace('/', '\\/');
    const regex = new RegExp(`uses:\\s*${escaped}@v(\\d+)`, 'g');
    for (const match of yamlText.matchAll(regex)) {
      const major = Number(match[1]);
      if (major < minMajor) {
        outdated.push(`${workflowName}: ${action}@v${major} (< v${minMajor})`);
      }
    }
  }
  return outdated;
}

function extractScopeLoop(ciYaml: string): string {
  const match = ciYaml.match(
    /(app_changed=false[\s\S]*?echo "site_changed=\$\{site_changed\}" >> "\$GITHUB_OUTPUT")/,
  );
  if (!match) {
    throw new Error('Could not locate Detect changed scopes loop in .github/workflows/ci.yml');
  }
  return match[1]
    .split('\n')
    .map((line) => line.replace(/^\s{10}/, ''))
    .join('\n')
    .replace('"${files[@]}"', '"$@"')
    .replace(/ >> "\$GITHUB_OUTPUT"/g, '');
}

function evaluateCiScopes(files: string[], ciYaml = readFileSync('.github/workflows/ci.yml', 'utf8')): Scopes {
  const script = `set -eu\n${extractScopeLoop(ciYaml)}\n`;
  const stdout = execFileSync('sh', ['-c', script, 'sh', ...files], { encoding: 'utf8' });
  const entries = Object.fromEntries(
    stdout
      .trim()
      .split('\n')
      .map((line) => line.split('=')),
  );
  return {
    appChanged: entries.app_changed === 'true',
    siteChanged: entries.site_changed === 'true',
  };
}

describe.skipIf(process.platform === 'win32')('CI 变更范围过滤与工作流门禁', () => {
  it('判据能区分免编译辅助路径与应用代码路径', () => {
    expect(
      evaluateCiScopes([
        'packaging/homebrew/Casks/dataomni.rb',
        'packaging/homebrew/README.md',
        'scripts/homebrew-cask.sh',
        'scripts/winget-manifest.sh',
        'glama.json',
        '.github/workflows/homebrew-tap.yml',
        '.github/workflows/windows-install.yml',
        'README.en.md',
        'TODOs.md',
        'rfcs/roadmap-1.3.md',
      ]),
    ).toEqual({
      appChanged: false,
      siteChanged: false,
    });

    expect(evaluateCiScopes(['site/src/docs.rs'])).toEqual({
      appChanged: false,
      siteChanged: true,
    });

    expect(evaluateCiScopes(['README.md'])).toEqual({
      appChanged: true,
      siteChanged: false,
    });

    expect(evaluateCiScopes(['scripts/install-macos.sh'])).toEqual({
      appChanged: true,
      siteChanged: false,
    });

    expect(evaluateCiScopes(['scripts/schema-diff-verify.sh'])).toEqual({
      appChanged: true,
      siteChanged: false,
    });

    expect(evaluateCiScopes(['src-tauri/src/lib.rs'])).toEqual({
      appChanged: true,
      siteChanged: false,
    });

    expect(evaluateCiScopes(['.github/workflows/ci.yml'])).toEqual({
      appChanged: true,
      siteChanged: true,
    });
  });

  it('判据能识别仍停留在 Node 20 的旧版官方 Action', () => {
    const sample = [
      '- uses: actions/checkout@v4',
      '- uses: actions/cache@v4',
      '- uses: actions/upload-artifact@v4',
      '- uses: actions/download-artifact@v4',
      '- uses: actions/upload-pages-artifact@v3',
      '- uses: actions/deploy-pages@v4',
      '- uses: actions/checkout@v7',
    ].join('\n');

    expect(findOutdatedNode20Actions('sample.yml', sample)).toEqual([
      'sample.yml: actions/checkout@v4 (< v7)',
      'sample.yml: actions/cache@v4 (< v6)',
      'sample.yml: actions/upload-artifact@v4 (< v7)',
      'sample.yml: actions/download-artifact@v4 (< v8)',
      'sample.yml: actions/upload-pages-artifact@v3 (< v5)',
      'sample.yml: actions/deploy-pages@v4 (< v5)',
    ]);
  });

  it('所有 GitHub Actions 工作流均已升级至原生 Node 24 主版本', () => {
    const dir = '.github/workflows';
    const findings = readdirSync(dir)
      .filter((name) => name.endsWith('.yml'))
      .sort()
      .flatMap((name) => findOutdatedNode20Actions(name, readFileSync(join(dir, name), 'utf8')));

    expect(findings).toEqual([]);
  });

  it('Homebrew Tap 自动同步工作流具备公证校验运行环境与预发布拦截护栏', () => {
    const tapWorkflow = readFileSync('.github/workflows/homebrew-tap.yml', 'utf8');
    // 必须在 macOS Runner 执行，否则 scripts/homebrew-cask.sh 里的 stapler/spctl 公证门禁不会生效
    expect(tapWorkflow).toContain('runs-on: macos-latest');
    expect(tapWorkflow).toContain('github.event.release.draft == false');
    expect(tapWorkflow).toContain('github.event.release.prerelease == false');
    expect(tapWorkflow).toContain('scripts/homebrew-cask.sh');
    expect(tapWorkflow).toContain('brew style packaging/homebrew/Casks/dataomni.rb');
    // Release published 事件的 GITHUB_REF 是 refs/tags/v*，绑定受保护 environment 会被分支白名单拒绝（见 11cb62e）
    expect(tapWorkflow).not.toMatch(/^\s*environment:/m);
  });

  it('build-latest-json.py 为正式版组装 macOS/Windows 签名清单并拒绝预发布、缺签名与 Linux 包混入', () => {
    const dir = mkdtempSync(join(tmpdir(), 'dataomni-latest-json-'));
    try {
      const sampleSig = Buffer.from(
        'untrusted comment: signature from tauri secret key\nRUTestSig\ntrusted comment: timestamp:1\tfile:pkg\nGlobalSig\n',
        'utf8',
      ).toString('base64');

      mkdirSync(join(dir, 'macos'), { recursive: true });
      mkdirSync(join(dir, 'nsis'), { recursive: true });
      mkdirSync(join(dir, 'msi'), { recursive: true });
      mkdirSync(join(dir, 'deb'), { recursive: true });

      writeFileSync(join(dir, 'macos', 'DataOmni.app.tar.gz'), 'mac-tar');
      writeFileSync(join(dir, 'macos', 'DataOmni.app.tar.gz.sig'), sampleSig);
      writeFileSync(join(dir, 'nsis', 'DataOmni_1.3.0_x64-setup.exe'), 'win-nsis');
      writeFileSync(join(dir, 'nsis', 'DataOmni_1.3.0_x64-setup.exe.sig'), sampleSig);
      writeFileSync(join(dir, 'msi', 'DataOmni_1.3.0_x64_en-US.msi'), 'win-msi');
      writeFileSync(join(dir, 'msi', 'DataOmni_1.3.0_x64_en-US.msi.sig'), sampleSig);
      writeFileSync(join(dir, 'deb', 'DataOmni_1.3.0_amd64.deb'), 'linux-deb');

      const notesPath = join(dir, 'notes.md');
      writeFileSync(notesPath, 'Release 1.3.0 notes\n');
      const outPath = join(dir, 'latest.json');

      execFileSync(
        'python3',
        [
          'scripts/build-latest-json.py',
          '--tag',
          'v1.3.0',
          '--repo',
          'dataomni-rs/DataOmni',
          '--packages-dir',
          dir,
          '--notes-file',
          notesPath,
          '--pub-date',
          '2026-11-19T08:00:00Z',
          '--output',
          outPath,
        ],
        { encoding: 'utf8' },
      );

      const manifest = JSON.parse(readFileSync(outPath, 'utf8')) as {
        version: string;
        pub_date: string;
        notes: string;
        platforms: Record<string, { url: string; signature: string }>;
      };

      expect(manifest.version).toBe('1.3.0');
      expect(manifest.pub_date).toBe('2026-11-19T08:00:00Z');
      expect(manifest.notes).toBe('Release 1.3.0 notes');
      expect(Object.keys(manifest.platforms).sort()).toEqual([
        'darwin-aarch64',
        'darwin-aarch64-app',
        'windows-x86_64',
        'windows-x86_64-msi',
        'windows-x86_64-nsis',
      ]);
      expect(manifest.platforms['darwin-aarch64'].url).toBe(
        'https://github.com/dataomni-rs/DataOmni/releases/download/v1.3.0/DataOmni.app.tar.gz',
      );
      expect(manifest.platforms['windows-x86_64-msi'].url).toBe(
        'https://github.com/dataomni-rs/DataOmni/releases/download/v1.3.0/DataOmni_1.3.0_x64_en-US.msi',
      );
      expect(manifest.platforms['windows-x86_64'].url).toBe(
        'https://github.com/dataomni-rs/DataOmni/releases/download/v1.3.0/DataOmni_1.3.0_x64-setup.exe',
      );

      // 反向验证 1：预发布 tag 绝不能生成正式通道的 latest.json
      expect(() =>
        execFileSync('python3', [
          'scripts/build-latest-json.py',
          '--tag',
          'v1.3.0-rc.1',
          '--repo',
          'dataomni-rs/DataOmni',
          '--packages-dir',
          dir,
          '--output',
          join(dir, 'rc.json'),
        ]),
      ).toThrow();

      // 反向验证 2：损坏的 .sig 签名文件必须报错中断
      writeFileSync(join(dir, 'macos', 'DataOmni.app.tar.gz.sig'), 'not-a-valid-minisign-sig');
      expect(() =>
        execFileSync('python3', [
          'scripts/build-latest-json.py',
          '--tag',
          'v1.3.0',
          '--repo',
          'dataomni-rs/DataOmni',
          '--packages-dir',
          dir,
          '--output',
          join(dir, 'bad.json'),
        ]),
      ).toThrow();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it('glama.json 符合 Glama MCP 目录认领格式且不含占位符', () => {
    const parsed = JSON.parse(readFileSync('glama.json', 'utf8')) as {
      $schema?: string;
      maintainers?: unknown;
    };
    expect(parsed.$schema).toBe('https://glama.ai/mcp/schemas/server.json');
    expect(Array.isArray(parsed.maintainers)).toBe(true);
    const maintainers = parsed.maintainers as unknown[];
    expect(maintainers.length).toBeGreaterThan(0);
    for (const handle of maintainers) {
      expect(typeof handle).toBe('string');
      expect((handle as string).trim().length).toBeGreaterThan(0);
      expect(handle).not.toMatch(/[<>]/);
    }
  });
});
