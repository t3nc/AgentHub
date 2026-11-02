/**
 * 复制文本：优先 Clipboard API，失败（非安全上下文 / 权限被拒）退回 textarea + execCommand。
 * 返回值让调用方决定提示文案，不在这里弹 toast。
 */
export async function copyText(text: string): Promise<boolean> {
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    return navigator.clipboard.writeText(text).then(
      () => true,
      (err) => {
        console.error("Clipboard API 不可用，改用 execCommand 兜底", err);
        return fallbackCopy(text);
      }
    );
  }
  return Promise.resolve(fallbackCopy(text));
}

function fallbackCopy(text: string): boolean {
  if (typeof document === "undefined") return false;
  const area = document.createElement("textarea");
  area.value = text;
  area.setAttribute("readonly", "");
  area.style.position = "fixed";
  area.style.top = "-1000px";
  area.style.opacity = "0";
  document.body.appendChild(area);
  area.select();
  let ok = false;
  try {
    ok = document.execCommand("copy");
  } catch (err) {
    console.error("复制失败", err);
  }
  document.body.removeChild(area);
  return ok;
}
