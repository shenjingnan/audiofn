import { describe, expect, it } from 'vitest';
import {
  PLATFORMS,
  RELEASE_BASE,
  RELEASES_PAGE,
  detectPlatform,
  platformByKey,
} from './downloads';

describe('detectPlatform', () => {
  it('windows 识别为 unknown（无 Windows 安装包，走 Releases 页兜底）', () => {
    expect(
      detectPlatform({
        ua: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)',
        platform: 'windows',
      }),
    ).toBe('unknown');
  });

  it('linux 经 userAgentData.platform 识别为 linux-x64', () => {
    expect(
      detectPlatform({
        ua: 'Mozilla/5.0 (X11; Linux x86_64)',
        platform: 'linux',
      }),
    ).toBe('linux-x64');
  });

  it('macos + arch arm → macos-arm64', () => {
    expect(
      detectPlatform({
        ua: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)',
        platform: 'macos',
        arch: 'arm',
      }),
    ).toBe('macos-arm64');
  });

  it('macos + arch x86 → macos-x64', () => {
    expect(
      detectPlatform({
        ua: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)',
        platform: 'macos',
        arch: 'x86',
      }),
    ).toBe('macos-x64');
  });

  it('macos + arch 缺失 → 默认 macos-arm64', () => {
    expect(
      detectPlatform({
        ua: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)',
        platform: 'macos',
      }),
    ).toBe('macos-arm64');
  });

  it('macos + arch unknown → 默认 macos-arm64', () => {
    expect(
      detectPlatform({
        ua: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)',
        platform: 'macos',
        arch: 'unknown',
      }),
    ).toBe('macos-arm64');
  });

  it('无 userAgentData：UA 含 Macintosh; Intel Mac OS X 不误判 Intel，返回 arm64', () => {
    // Apple Silicon 上的 Safari/Firefox 出于兼容也上报 "Intel Mac OS X"
    expect(
      detectPlatform({
        ua: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15',
      }),
    ).toBe('macos-arm64');
  });

  it('无 userAgentData：UA 回退 Windows → unknown（无 Windows 安装包）', () => {
    expect(
      detectPlatform({
        ua: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36',
      }),
    ).toBe('unknown');
  });

  it('无 userAgentData：UA 回退 Linux → linux-x64', () => {
    expect(
      detectPlatform({
        ua: 'Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:126.0) Gecko/20100101 Firefox/126.0',
      }),
    ).toBe('linux-x64');
  });

  it('Android → unknown', () => {
    expect(
      detectPlatform({
        ua: 'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36',
        platform: 'android',
      }),
    ).toBe('unknown');
  });

  it('iPhone → unknown', () => {
    expect(
      detectPlatform({
        ua: 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X)',
        platform: 'iphone',
      }),
    ).toBe('unknown');
  });

  it('空 UA → unknown', () => {
    expect(detectPlatform({ ua: '' })).toBe('unknown');
  });
});

describe('PLATFORMS 数据完整性', () => {
  it('每个平台至少有一个下载文件', () => {
    for (const p of PLATFORMS) {
      expect(p.files.length, `${p.key} 缺少下载文件`).toBeGreaterThan(0);
    }
  });

  it('文件名全局唯一', () => {
    const names = PLATFORMS.flatMap((p) => p.files.map((f) => f.fileName));
    expect(new Set(names).size).toBe(names.length);
  });

  it('所有直链以 RELEASE_BASE 开头', () => {
    for (const p of PLATFORMS) {
      for (const f of p.files) {
        expect(f.url.startsWith(RELEASE_BASE)).toBe(true);
        expect(f.url).not.toBe(RELEASES_PAGE);
      }
    }
  });

  it('文件名集合为页面展示的下载文件（与 release.yml 的 Rename 步骤严格同步）', () => {
    const names = PLATFORMS.flatMap((p) => p.files.map((f) => f.fileName)).sort();
    expect(names).toEqual([
      'AudioFn_Linux_amd64.AppImage',
      'AudioFn_Linux_amd64.deb',
      'AudioFn_Linux_x86_64.rpm',
      'AudioFn_macOS_arm64.dmg',
      'AudioFn_macOS_x64.dmg',
    ]);
  });

  it('不提供 Windows 安装包（构建白名单仅 macOS / Linux）', () => {
    expect(PLATFORMS.map((p) => p.os)).not.toContain('Windows');
    const names = PLATFORMS.flatMap((p) => p.files.map((f) => f.fileName));
    expect(names.some((n) => n.endsWith('.exe') || n.endsWith('.msi'))).toBe(false);
  });

  it('Linux 每个格式都有适用系统说明', () => {
    const linux = platformByKey('linux-x64');
    for (const f of linux?.files ?? []) {
      expect(f.systems, `${f.label} 缺少适用系统说明`).toBeTruthy();
    }
  });

  it('macOS 平台的 note 指引双击「首次打开修复.command」', () => {
    // 文件名须与 dmg 内注入的修复脚本（scripts/patch-dmg-gatekeeper.sh 的
    // FIXER_NAME）及 README「macOS 首次打开」保持同步
    for (const p of PLATFORMS.filter((p) => p.os === 'macOS')) {
      expect(p.note ?? '', `${p.key} 的 note 未提及修复脚本`).toContain(
        '首次打开修复.command',
      );
    }
  });
});

describe('platformByKey', () => {
  it('返回对应平台', () => {
    expect(platformByKey('macos-arm64')?.os).toBe('macOS');
    expect(platformByKey('linux-x64')?.arch).toBe('x86_64');
  });

  it('unknown 不命中任何平台', () => {
    expect(platformByKey('unknown')).toBeUndefined();
  });
});
