// A small W3C WebDriver client, so end-to-end tests need no extra packages.
const ELEMENT = "element-6066-11e4-a52e-4f735466cecf";

export class Driver {
  constructor(base = "http://127.0.0.1:4444") {
    this.base = base;
    this.session = null;
  }

  async request(method, path, body) {
    const response = await fetch(`${this.base}${path}`, {
      method,
      headers: { "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const payload = await response.json();
    if (!response.ok) throw new Error(`${method} ${path}: ${JSON.stringify(payload.value)}`);
    return payload.value;
  }

  async start(application) {
    const value = await this.request("POST", "/session", {
      capabilities: { alwaysMatch: { browserName: "wry", "tauri:options": { application } } },
    });
    this.session = value.sessionId;
  }

  async stop() {
    await this.request("DELETE", `/session/${this.session}`);
  }

  command(method, path, body) {
    return this.request(method, `/session/${this.session}${path}`, body);
  }

  async find(xpath, timeout = 8000) {
    const deadline = Date.now() + timeout;
    let failure;
    while (Date.now() < deadline) {
      try {
        const element = await this.command("POST", "/element", { using: "xpath", value: xpath });
        return element[ELEMENT];
      } catch (error) {
        failure = error;
        await new Promise((resolve) => setTimeout(resolve, 150));
      }
    }
    throw new Error(`Not found: ${xpath}\n${failure}`);
  }

  async findAll(xpath) {
    const elements = await this.command("POST", "/elements", { using: "xpath", value: xpath });
    return elements.map((element) => element[ELEMENT]);
  }

  async click(xpath) {
    await this.command("POST", `/element/${await this.find(xpath)}/click`, {});
  }

  async clickText(text) {
    await this.click(`//*[self::button or self::a][normalize-space()="${text}"]`);
  }

  async type(xpath, text) {
    const element = await this.find(xpath);
    await this.command("POST", `/element/${element}/clear`, {});
    await this.command("POST", `/element/${element}/value`, { text });
  }

  async text(xpath) {
    return this.command("GET", `/element/${await this.find(xpath)}/text`);
  }

  async seesText(text, timeout) {
    await this.find(`//*[contains(normalize-space(.), "${text}")]`, timeout);
  }

  async visit(page) {
    await this.command("POST", "/execute/sync", { script: `window.location.hash = "${page}"`, args: [] });
  }

  async script(source) {
    return this.command("POST", "/execute/sync", { script: source, args: [] });
  }

  async screenshot(path) {
    const { writeFile } = await import("node:fs/promises");
    await writeFile(path, Buffer.from(await this.command("GET", "/screenshot"), "base64"));
  }
}
