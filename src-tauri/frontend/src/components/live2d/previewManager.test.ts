import { beforeEach, describe, expect, it, vi } from "vitest";

/**
 * previewManager 单元测试：mock pixi.js / pixi-live2d-display / modelLayout，
 * 只验证管理器状态机（claim/release/showModel/LRU/ticker 治理），不碰真实 WebGL。
 */
const { ApplicationMock, appInstances, fromMock, modelInstances, boundsMock, layoutMock } =
  vi.hoisted(() => {
    // 每次构造都生成可断言的 stub 实例，数组跨 hoisted 收集供测试引用。
    const appInstances: Array<{
      view: HTMLCanvasElement;
      ticker: { start: ReturnType<typeof vi.fn>; stop: ReturnType<typeof vi.fn> };
      renderer: { resize: ReturnType<typeof vi.fn> };
      stage: { addChild: ReturnType<typeof vi.fn>; removeChild: ReturnType<typeof vi.fn> };
      destroy: ReturnType<typeof vi.fn>;
    }> = [];
    const modelInstances: Array<{
      url: string;
      autoUpdate: boolean;
      destroy: ReturnType<typeof vi.fn>;
      internalModel: {
        motionManager: {
          definitions: Partial<Record<string, { File: string }[]>>;
          startMotion: ReturnType<typeof vi.fn>;
        };
        expressionManager?: {
          definitions: { Name: string }[];
          setExpression: ReturnType<typeof vi.fn>;
          resetExpression: ReturnType<typeof vi.fn>;
        };
      };
    }> = [];
    // 注意：实现必须用普通 function（可构造）——生产代码以 new PIXI.Application() 调用，
    // 箭头函数实现在此 Vitest 版本下 new 时抛 "is not a constructor"。
    // biome-ignore lint/complexity/useArrowFunction: 此 mock 以 new 调用，箭头函数不可构造
    const ApplicationMock = vi.fn(function () {
      const app = {
        view: document.createElement("canvas"),
        ticker: { start: vi.fn(), stop: vi.fn() },
        renderer: { resize: vi.fn() },
        stage: { addChild: vi.fn(), removeChild: vi.fn() },
        destroy: vi.fn(),
      };
      appInstances.push(app);
      return app;
    });
    const fromMock = vi.fn(async (url: string) => {
      const model = {
        url,
        autoUpdate: false,
        destroy: vi.fn(),
        internalModel: {
          motionManager: {
            definitions:
              url === "asset://empty.model3.json"
                ? {}
                : {
                    Extra: [
                      { File: "Motions/睡觉动画.motion3.json" },
                      { File: "chibang.motion3.json" },
                    ],
                  },
            startMotion: vi.fn(async () => true),
          },
          expressionManager:
            url === "asset://empty.model3.json"
              ? undefined
              : {
                  definitions: [
                    { Name: "03 生气", File: "a.exp3.json" },
                    { Name: "07 星星眼", File: "b.exp3.json" },
                  ],
                  setExpression: vi.fn(async () => true),
                  resetExpression: vi.fn(),
                },
        },
      };
      modelInstances.push(model);
      return model;
    });
    // computeModelBounds 返回恒定合法包围盒，使 metrics 回调路径可触发。
    const boundsMock = vi.fn(() => ({ cx: 0, cy: 0, width: 100, height: 200 }));
    const layoutMock = vi.fn();
    return { ApplicationMock, appInstances, fromMock, modelInstances, boundsMock, layoutMock };
  });

vi.mock("pixi.js", () => ({ Application: ApplicationMock }));
vi.mock("pixi-live2d-display/cubism4", () => ({ Live2DModel: { from: fromMock } }));
vi.mock("./modelLayout", () => ({ computeModelBounds: boundsMock, layoutModel: layoutMock }));

/** 每个用例重置模块，拿到全新单例（不引入 test-only reset 钩子）。 */
async function freshManager() {
  vi.resetModules();
  const mod = await import("./previewManager");
  return mod.getPreviewManager();
}

function makeSlot() {
  const el = document.createElement("div");
  document.body.appendChild(el);
  return el;
}

function makeCallbacks() {
  return {
    onError: vi.fn(),
    onModelMetrics: vi.fn(),
    onModelReady: vi.fn(),
    onModelCatalog: vi.fn(),
  };
}

type Manager = Awaited<ReturnType<typeof freshManager>>;

function claim(manager: Manager) {
  const element = makeSlot();
  const callbacks = makeCallbacks();
  const handle = manager.claim({ element, width: 300, height: 200, modelScale: 1, callbacks });
  return { element, callbacks, handle };
}

beforeEach(() => {
  vi.clearAllMocks();
  appInstances.length = 0;
  modelInstances.length = 0;
  // 可见性用例会覆写 document.hidden，这里恢复默认，避免断言失败时泄漏到后续用例。
  Object.defineProperty(document, "hidden", { value: false, configurable: true });
});

describe("PreviewManager claim/release", () => {
  it("首次 claim 懒创建唯一 app：canvas 挂进 slot、ticker 启动、renderer 按尺寸 resize", async () => {
    const manager = await freshManager();
    const { element, handle } = claim(manager);

    expect(ApplicationMock).toHaveBeenCalledTimes(1);
    expect(ApplicationMock).toHaveBeenCalledWith(
      expect.objectContaining({ autoStart: false, backgroundAlpha: 0, antialias: true }),
    );
    const app = appInstances[0];
    expect(element.contains(app.view)).toBe(true);
    expect(app.ticker.start).toHaveBeenCalled();
    expect(app.renderer.resize).toHaveBeenCalledWith(300, 200);
    handle.release();
  });

  it("二次 claim 不再创建 app：canvas 移到新 slot；旧 handle 的 release 是 no-op", async () => {
    const manager = await freshManager();
    const a = claim(manager);
    const b = claim(manager);

    expect(ApplicationMock).toHaveBeenCalledTimes(1);
    expect(b.element.contains(appInstances[0].view)).toBe(true);
    expect(a.element.contains(appInstances[0].view)).toBe(false);

    // 旧 handle 的 release 不应把 canvas 抢回 parking、不应停新 handle 的 ticker。
    a.handle.release();
    expect(b.element.contains(appInstances[0].view)).toBe(true);
    expect(appInstances[0].ticker.stop).not.toHaveBeenCalled();
    b.handle.release();
  });

  it("release 当前 handle：canvas 回 parking（脱离 slot）、ticker 停止", async () => {
    const manager = await freshManager();
    const { element, handle } = claim(manager);
    handle.release();

    const app = appInstances[0];
    expect(element.contains(app.view)).toBe(false);
    expect(document.body.contains(app.view)).toBe(true);
    expect(app.ticker.stop).toHaveBeenCalled();
  });

  it("claim→release→claim（StrictMode 双挂载）幂等：app 恒为 1 个，ticker 重新启动", async () => {
    const manager = await freshManager();
    const first = claim(manager);
    first.handle.release();
    const second = claim(manager);

    expect(ApplicationMock).toHaveBeenCalledTimes(1);
    expect(second.element.contains(appInstances[0].view)).toBe(true);
    expect(appInstances[0].ticker.start).toHaveBeenCalledTimes(2);
    second.handle.release();
  });

  it("app 创建失败：claim 回调 onError；下次 claim 重试创建成功", async () => {
    const manager = await freshManager();
    // biome-ignore lint/complexity/useArrowFunction: 以 new 调用的 mock，须可构造
    ApplicationMock.mockImplementationOnce(function () {
      throw new Error("boom");
    });
    const fail = claim(manager);
    expect(fail.callbacks.onError).toHaveBeenCalledWith(
      expect.objectContaining({ message: "boom" }),
    );

    const ok = claim(manager);
    expect(ApplicationMock).toHaveBeenCalledTimes(2);
    expect(ok.element.contains(appInstances[0].view)).toBe(true);
    ok.handle.release();
  });
});

describe("PreviewManager showModel", () => {
  it("加载模型上舞台：from 调用一次、addChild、autoUpdate 开启、布局并回调 metrics/ready", async () => {
    const manager = await freshManager();
    const { callbacks, handle } = claim(manager);

    handle.showModel("asset://a.model3.json", "k1");
    await vi.waitFor(() => expect(callbacks.onModelReady).toHaveBeenCalled());

    expect(fromMock).toHaveBeenCalledTimes(1);
    expect(fromMock).toHaveBeenCalledWith("asset://a.model3.json", { autoInteract: false });
    const model = modelInstances[0];
    const app = appInstances[0];
    expect(app.stage.addChild).toHaveBeenCalledWith(model);
    expect(model.autoUpdate).toBe(true);
    expect(layoutMock).toHaveBeenCalledWith(model, 300, 200, 1);
    expect(callbacks.onModelMetrics).toHaveBeenCalledWith({ aspectRatio: 0.5 });
    expect(callbacks.onModelReady).toHaveBeenCalledWith(app.view);
    handle.release();
  });

  it("缓存命中：另一 handle show 同 url 同 reloadKey → from 不再调用、同一实例上舞台、ready 再次回调", async () => {
    const manager = await freshManager();
    const first = claim(manager);
    first.handle.showModel("asset://a.model3.json", "k1");
    await vi.waitFor(() => expect(first.callbacks.onModelReady).toHaveBeenCalled());
    first.handle.release();

    const second = claim(manager);
    second.handle.showModel("asset://a.model3.json", "k1");
    expect(fromMock).toHaveBeenCalledTimes(1); // 缓存命中，零加载
    expect(second.callbacks.onModelReady).toHaveBeenCalledWith(appInstances[0].view);
    expect(modelInstances[0].autoUpdate).toBe(true);
    second.handle.release();
  });

  it("同 handle 重复 show 同 url 同 key：no-op（不重复 addChild / 不重复回调）", async () => {
    const manager = await freshManager();
    const { callbacks, handle } = claim(manager);
    handle.showModel("asset://a.model3.json", "k1");
    await vi.waitFor(() => expect(callbacks.onModelReady).toHaveBeenCalled());
    const readyCalls = callbacks.onModelReady.mock.calls.length;

    handle.showModel("asset://a.model3.json", "k1");
    expect(callbacks.onModelReady.mock.calls.length).toBe(readyCalls);
    expect(appInstances[0].stage.addChild).toHaveBeenCalledTimes(1);
    handle.release();
  });

  it("reloadKey 变化强制重载：旧实例 destroy、from 重新调用", async () => {
    const manager = await freshManager();
    const { callbacks, handle } = claim(manager);
    handle.showModel("asset://a.model3.json", "k1");
    // 等 onModelReady（attach 完成）再触发 reloadKey，避免微任务时序竞争。
    await vi.waitFor(() => expect(callbacks.onModelReady).toHaveBeenCalled());

    handle.showModel("asset://a.model3.json", "k2");
    await vi.waitFor(() => expect(modelInstances.length).toBe(2));

    expect(modelInstances[0].destroy).toHaveBeenCalled();
    expect(fromMock).toHaveBeenCalledTimes(2);
    handle.release();
  });

  it("showModel(null)：模型下舞台（仍在缓存）、autoUpdate 关闭、不 destroy", async () => {
    const manager = await freshManager();
    const { callbacks, handle } = claim(manager);
    handle.showModel("asset://a.model3.json", "k1");
    // 等 onModelReady（attach 完成）再清屏，否则清屏跑在挂载之前。
    await vi.waitFor(() => expect(callbacks.onModelReady).toHaveBeenCalled());

    handle.showModel(null, "k1");
    const app = appInstances[0];
    expect(app.stage.removeChild).toHaveBeenCalledWith(modelInstances[0]);
    expect(modelInstances[0].autoUpdate).toBe(false);
    expect(modelInstances[0].destroy).not.toHaveBeenCalled();

    // 清屏后再 show 同 url：缓存仍在，零加载。
    handle.showModel("asset://a.model3.json", "k1");
    expect(fromMock).toHaveBeenCalledTimes(1);
    expect(app.stage.addChild).toHaveBeenCalledTimes(2);
    handle.release();
  });

  it("LRU cap=3：第 4 个模型逐出最旧并 destroy；中间模型不销毁、被逐出者再 show 需重载", async () => {
    const manager = await freshManager();
    const { handle } = claim(manager);
    for (const url of ["asset://m1", "asset://m2", "asset://m3"]) {
      handle.showModel(url, "k");
      await vi.waitFor(() => expect(layoutMock).toHaveBeenCalled());
      layoutMock.mockClear();
    }
    expect(modelInstances).toHaveLength(3);

    handle.showModel("asset://m4", "k");
    await vi.waitFor(() => expect(modelInstances).toHaveLength(4));

    // m1（最旧）被逐出销毁；m2/m3 仍在缓存。
    expect(modelInstances[0].destroy).toHaveBeenCalled();
    expect(modelInstances[1].destroy).not.toHaveBeenCalled();
    expect(modelInstances[2].destroy).not.toHaveBeenCalled();

    handle.showModel("asset://m2", "k");
    expect(fromMock).toHaveBeenCalledTimes(4); // m2 缓存命中，未新增加载
    handle.showModel("asset://m1", "k");
    await vi.waitFor(() => expect(modelInstances).toHaveLength(5)); // m1 被逐出过，需重载
    handle.release();
  });

  it("加载期间 release：完成后只入缓存不上舞台、不回调；随后新 handle show 命中缓存", async () => {
    const manager = await freshManager();
    const first = claim(manager);

    type LoadedModel = Awaited<ReturnType<typeof fromMock>>;
    let resolveModel: (m: LoadedModel) => void = () => {};
    fromMock.mockImplementationOnce(
      () =>
        new Promise<LoadedModel>((resolve) => {
          resolveModel = resolve;
        }),
    );
    first.handle.showModel("asset://slow.model3.json", "k1");
    first.handle.release();

    resolveModel({
      url: "asset://slow.model3.json",
      autoUpdate: false,
      destroy: vi.fn(),
      internalModel: {
        motionManager: { definitions: {}, startMotion: vi.fn(async () => true) },
        expressionManager: undefined,
      },
    });
    // 冲刷微任务，确保加载完成路径（只入缓存、不上舞台）已执行。
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(appInstances[0].stage.addChild).not.toHaveBeenCalled();
    expect(first.callbacks.onModelReady).not.toHaveBeenCalled();

    const second = claim(manager);
    second.handle.showModel("asset://slow.model3.json", "k1");
    expect(fromMock).toHaveBeenCalledTimes(1); // 命中刚完成的缓存
    expect(appInstances[0].stage.addChild).toHaveBeenCalledWith(
      expect.objectContaining({ url: "asset://slow.model3.json" }),
    );
    second.handle.release();
  });

  it("加载失败：回调 onError；失败不缓存，重试会重新调用 from", async () => {
    const manager = await freshManager();
    const { callbacks, handle } = claim(manager);
    fromMock.mockRejectedValueOnce(new Error("moc broken"));

    handle.showModel("asset://bad.model3.json", "k1");
    await vi.waitFor(() => expect(callbacks.onError).toHaveBeenCalled());
    expect(callbacks.onError).toHaveBeenCalledWith(
      expect.objectContaining({ message: "moc broken" }),
    );

    handle.showModel("asset://bad.model3.json", "k1");
    await vi.waitFor(() => expect(fromMock).toHaveBeenCalledTimes(2));
    handle.release();
  });
});

describe("PreviewManager updateLayout", () => {
  it("resize 渲染器并按新尺寸/缩放重新布局当前模型", async () => {
    const manager = await freshManager();
    const { handle } = claim(manager);
    handle.showModel("asset://a.model3.json", "k1");
    await vi.waitFor(() => expect(layoutMock).toHaveBeenCalled());
    layoutMock.mockClear();

    handle.updateLayout(500, 400, 0.8);
    expect(appInstances[0].renderer.resize).toHaveBeenCalledWith(500, 400);
    expect(layoutMock).toHaveBeenCalledWith(modelInstances[0], 500, 400, 0.8);
    handle.release();
  });
});

describe("PreviewManager 可见性治理", () => {
  function setHidden(hidden: boolean) {
    Object.defineProperty(document, "hidden", { value: hidden, configurable: true });
    document.dispatchEvent(new Event("visibilitychange"));
  }

  it("文档隐藏：ticker 停 + 模型 autoUpdate 关；重新可见：恢复", async () => {
    const manager = await freshManager();
    const { handle } = claim(manager);
    handle.showModel("asset://a.model3.json", "k1");
    await vi.waitFor(() => expect(modelInstances[0].autoUpdate).toBe(true));

    const app = appInstances[0];
    app.ticker.start.mockClear();
    app.ticker.stop.mockClear();

    setHidden(true);
    expect(app.ticker.stop).toHaveBeenCalled();
    expect(modelInstances[0].autoUpdate).toBe(false);

    setHidden(false);
    expect(app.ticker.start).toHaveBeenCalled();
    expect(modelInstances[0].autoUpdate).toBe(true);
    handle.release();
    setHidden(false);
  });
});

describe("PreviewManager 动作/表情目录与播放", () => {
  it("attach 后回调 onModelCatalog（含展示名派生）", async () => {
    const manager = await freshManager();
    const { callbacks, handle } = claim(manager);
    handle.showModel("asset://a.model3.json", "default");
    await vi.waitFor(() => expect(callbacks.onModelCatalog).toHaveBeenCalled());
    expect(callbacks.onModelCatalog).toHaveBeenCalledWith({
      motionGroups: [
        {
          group: "Extra",
          motions: [
            { index: 0, name: "睡觉动画" },
            { index: 1, name: "chibang" },
          ],
        },
      ],
      expressions: [
        { index: 0, name: "03 生气" },
        { index: 1, name: "07 星星眼" },
      ],
    });
  });

  it("playMotion 以 FORCE 优先级转发；applyExpression/resetExpression 走 index；release 后 no-op", async () => {
    const manager = await freshManager();
    const { callbacks, handle } = claim(manager);
    handle.showModel("asset://a.model3.json", "default");
    // onModelCatalog 是 attach（模型上舞台、shownModel 就绪）的完成信号。
    await vi.waitFor(() => expect(callbacks.onModelCatalog).toHaveBeenCalled());
    const model = modelInstances[0];

    expect(await handle.playMotion("Extra", 1)).toBe(true);
    expect(model.internalModel.motionManager.startMotion).toHaveBeenCalledWith("Extra", 1, 3);

    expect(await handle.applyExpression(0)).toBe(true);
    expect(model.internalModel.expressionManager?.setExpression).toHaveBeenCalledWith(0);

    handle.resetExpression();
    expect(model.internalModel.expressionManager?.resetExpression).toHaveBeenCalled();

    // 释放后（不再是当前占用者）播放安全 no-op。
    handle.release();
    expect(await handle.playMotion("Extra", 0)).toBe(false);
  });

  it("空目录模型回调空 catalog（两组空数组，非 null）", async () => {
    const manager = await freshManager();
    const { callbacks, handle } = claim(manager);
    handle.showModel("asset://empty.model3.json", "default");
    await vi.waitFor(() => expect(callbacks.onModelCatalog).toHaveBeenCalled());
    expect(callbacks.onModelCatalog).toHaveBeenLastCalledWith({
      motionGroups: [],
      expressions: [],
    });
  });
});
