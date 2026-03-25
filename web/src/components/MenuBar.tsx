export default function MenuBar() {
  return (
    <header className="menubar">
      <span className="menubar-title">ALSIE</span>
      <span className="menubar-sep" />
      <button className="menubar-icon-btn" title="Run">▶</button>
      <button className="menubar-icon-btn" title="Stop">■</button>
      <button className="menubar-icon-btn" title="Settings">⚙</button>
    </header>
  );
}
