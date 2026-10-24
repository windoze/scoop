const pages = new Map([
  ["/", ["首页", "这份页面由 public/index.html 提供。"]],
  [
    "/settings/profile",
    [
      "个人设置",
      "服务器上没有这个路径的文件。刷新时由 fallbackFile 返回同一份 SPA 入口。",
    ],
  ],
  [
    "/about",
    ["关于", "路由切换由浏览器 History API 完成，不需要重新下载整个页面。"],
  ],
]);

function render() {
  const path = window.location.pathname;
  const page = pages.get(path) ?? [
    "页面未登记",
    "服务器已返回 SPA 入口，由客户端决定如何显示这个路径。",
  ];
  document.querySelector("#page-title").textContent = page[0];
  document.querySelector("#page-description").textContent = page[1];
  document.querySelector("#page-path").textContent = path;
  for (const link of document.querySelectorAll("[data-route]")) {
    if (link.getAttribute("href") === path)
      link.setAttribute("aria-current", "page");
    else link.removeAttribute("aria-current");
  }
}

for (const link of document.querySelectorAll("[data-route]")) {
  link.addEventListener("click", (event) => {
    if (
      event.button !== 0 ||
      event.metaKey ||
      event.ctrlKey ||
      event.shiftKey ||
      event.altKey
    )
      return;
    event.preventDefault();
    window.history.pushState(null, "", link.getAttribute("href"));
    render();
  });
}

window.addEventListener("popstate", render);

document.querySelector("#load-user").addEventListener("click", async () => {
  const output = document.querySelector("#api-result");
  output.textContent = "正在查询…";
  try {
    const response = await fetch("/api/v1/users/42");
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    output.textContent = await response.text();
  } catch (error) {
    output.textContent = `请求失败：${String(error)}`;
  }
});

render();
