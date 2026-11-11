import { POCKET_BINDING_GENERATION } from "../contracts/spec/binding-generation.ts";
import { afterAll, describe, expect, test } from "bun:test";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  FRAMEWORKS,
  RENDERER_OCTANE_PATH,
  RENDERER_PATH,
  RENDERER_SOLID_PATH,
  RENDERER_VUE_VAPOR_PATH,
  frameworkVariantPath,
  jsxPlugin,
  packagePath,
  transformFile,
} from "../framework/compiler/jsx-plugin.ts";

const directories: string[] = [];

afterAll(async () => {
  await Promise.all(directories.map((directory) => rm(directory, { recursive: true, force: true })));
});

async function temporaryDirectory(prefix: string): Promise<string> {
  const directory = await mkdtemp(join(tmpdir(), prefix));
  directories.push(directory);
  return directory;
}

function expectNativeFile(path: string): void {
  if (process.platform === "win32") {
    expect(path).toMatch(/^(?:[A-Za-z]:\\|\\\\)/);
    expect(path).not.toMatch(/^\/[A-Za-z]:\//);
  } else {
    expect(path.startsWith("/")).toBe(true);
  }
}

describe("compiler filesystem portability", () => {
  test("compiler-owned renderer and package paths are native files", () => {
    for (const path of [
      RENDERER_PATH,
      RENDERER_SOLID_PATH,
      RENDERER_VUE_VAPOR_PATH,
      RENDERER_OCTANE_PATH,
    ]) {
      expectNativeFile(path);
    }

    for (const framework of ["solid", "vue-vapor", "octane"] as const) {
      expectNativeFile(FRAMEWORKS[framework].rootPath);
      expectNativeFile(FRAMEWORKS[framework].rendererPath);
      const components = packagePath("@pocketjs/framework/components", framework);
      expect(components).not.toBeNull();
      expectNativeFile(components!);
    }
  });

  test("the transform cache accepts compiler output on the native filesystem", async () => {
    const source = "export const view = <View>Cache probe</View>;";
    const first = await transformFile("/virtual/compiler-cache-probe.tsx", source, "solid");
    const second = await transformFile("/virtual/compiler-cache-probe.tsx", source, "solid");

    expect(second.code).toBe(first.code);
    expect(second.classStrings).toEqual(first.classStrings);
    expect(second.textCodepoints).toEqual(first.textCodepoints);
  });

  test.each([
    ["solid", 'import { createSignal } from "solid-js"; export const value = createSignal(1);'],
    ["vue-vapor", 'import { ref } from "vue"; export const value = ref(1);'],
  ] as const)("%s runtime alias resolves to a bundleable native file", async (framework, source) => {
    const directory = await temporaryDirectory(`pocketjs-${framework}-runtime-`);
    const entry = join(directory, "entry.js");
    await Bun.write(entry, source);

    const result = await Bun.build({
      entrypoints: [entry],
      format: "esm",
      target: "browser",
      conditions: ["browser"],
      define: {
        "process.env.NODE_ENV": '"production"',
        document: "globalThis.__pocketDocument",
        __POCKET_TARGET__: '""',
        __POCKET_HOST_ABI__: String(POCKET_BINDING_GENERATION),
        __POCKET_FEATURES__: "{}",
        __POCKET_PIXEL_RATIO__: "1",
      },
      plugins: [jsxPlugin(framework, { entry })],
    });

    expect(result.success).toBe(true);
    expect((await result.outputs[0]!.text()).length).toBeGreaterThan(0);
  });

  test("LF and CRLF JSXText produce equivalent compiler output", async () => {
    const lf = "export const view = <View>\n  Hello\n</View>;\n";
    const crlf = lf.replace(/\n/g, "\r\n");
    const fromLf = await transformFile("/virtual/line-endings.tsx", lf, "solid");
    const fromCrlf = await transformFile("/virtual/line-endings.tsx", crlf, "solid");

    expect(fromCrlf.code).toBe(fromLf.code);
    expect(fromCrlf.classStrings).toEqual(fromLf.classStrings);
    expect(fromCrlf.textCodepoints).toEqual(fromLf.textCodepoints);
  });

  test.each([
    ["named", "&eacute;"],
    ["numeric LF", "&#10;"],
    ["numeric CR", "&#13;"],
  ])("rejects %s entities in JSXText", async (name, entity) => {
    await expect(
      transformFile(`/virtual/entity-${name}.tsx`, `<View>${entity}</View>`, "solid"),
    ).rejects.toThrow("HTML entities in JSX text are not decoded");
  });

  test("native node_modules separators do not select framework variants", async () => {
    const directory = await temporaryDirectory("pocketjs-node-modules-path-");
    const moduleDirectory = process.platform === "win32"
      ? join(directory, "node_modules", "fixture")
      : directory;
    await mkdir(moduleDirectory, { recursive: true });
    const source = process.platform === "win32"
      ? join(moduleDirectory, "entry.ts")
      : join(moduleDirectory, String.raw`C:\repo\node_modules\fixture\entry.ts`);
    const variant = source.replace(/\.ts$/, ".octane.ts");
    await Bun.write(source, "export const selected = 'package';\n");
    await Bun.write(variant, "export const selected = 'variant';\n");

    expect(frameworkVariantPath(source, "octane")).toBe(source);
  });
});


test("scalar text optimization keeps conditional component ownership", async () => {
  const directory = await temporaryDirectory("pocketjs-scalar-ownership-");
  const entry = join(directory, "entry.tsx");
  await Bun.write(entry, `
    import { createSignal, onCleanup } from "solid-js";
    import { render } from "@pocketjs/framework/solid";
    let mounts = 0, cleanups = 0;
    const [value, set] = createSignal(1);
    function Child() { mounts++; onCleanup(() => cleanups++); return <text>child</text>; }
    const dispose = render(() => <view><text>{value() > 0 ? "up" : "down"}</text>{value() > 0 ? <Child /> : <text>gone</text>}</view>, { ops: (globalThis as any).__scalarHost });
    (globalThis as any).__scalarFixture = { set, dispose, counts: () => [mounts, cleanups] };
  `);
  const result = await Bun.build({ entrypoints: [entry], format: "iife", target: "browser", conditions: ["browser"],
    define: { "process.env.NODE_ENV": '"production"', __POCKET_TARGET__: '""', __POCKET_HOST_ABI__: String(POCKET_BINDING_GENERATION), __POCKET_FEATURES__: "{}", __POCKET_PIXEL_RATIO__: "1" },
    plugins: [jsxPlugin("solid", { entry })] });
  expect(result.success).toBe(true);
  const g = globalThis as any;
  let next = 2;
  const text: string[] = [];
  g.__scalarHost = { createNode: () => next++, destroyNode() {}, insertBefore() {}, removeChild() {},
    setStyle() {}, setProp() {}, setText(_id: number, value: string) { text.push(value); },
    replaceText(_id: number, value: string) { text.push(value); }, setFocus() {} };
  try {
    (0, eval)(await result.outputs[0]!.text());
    const fixture = g.__scalarFixture;
    expect(fixture.counts()).toEqual([1, 0]);
    fixture.set(2); expect(fixture.counts()).toEqual([1, 0]);
    fixture.set(0); expect(fixture.counts()).toEqual([1, 1]); expect(text).toContain("down");
    fixture.set(1); expect(fixture.counts()).toEqual([2, 1]); expect(text).toContain("up");
    fixture.dispose(); expect(fixture.counts()).toEqual([2, 2]);
  } finally {
    delete g.__scalarHost; delete g.__scalarFixture;
  }
});

test("custom Text components retain ordinary conditional compilation", async () => {
  const source = 'function Text(props: any) { return props.children; } export function App(props: { value: number }) { return <Text>{props.value > 0 ? "yes" : "no"}</Text>; }';
  const transformed = await transformFile("/virtual/custom-text-owner.tsx", source, "solid");
  expect(transformed.code).toMatch(/memo/);
});


test("literal native classes compile to retained bindings without lowering dynamic classes", async () => {
  const source = `export function App(props: { active: boolean, dynamic: string }) {
    return <view class="p-2"><view class={props.active ? "bg-red-500" : "bg-blue-500"} /><view class={props.dynamic} /></view>;
  }`;
  const result = await transformFile("/virtual/retained-classes.tsx", source, "solid");
  expect(result.code).toContain("createClassBinding");
  expect(result.code).toContain("setClassBinding");
  expect(result.code).toContain('"class",');
  expect(result.classStrings).toContain("p-2");
  expect(result.classStrings).toContain("bg-red-500");
  expect(result.classStrings).toContain("bg-blue-500");
  const manual = await transformFile("/virtual/manual-setter.ts", `import {setProp} from "${RENDERER_SOLID_PATH}"; export function set(node: any) { setProp(node, "class", "p-2"); }`, "solid");
  expect(manual.code).not.toContain("ClassBinding");
});
