export default function MenuBar() {
  const menus = ['File', 'Edit', 'View', 'Run', 'Help'];

  return (
    <header className="menubar">
      <span className="menubar-title">ALSIE</span>
      {menus.map((m) => (
        <button key={m} className="menubar-btn">{m}</button>
      ))}
      <span className="menubar-sep" />
      <button className="menubar-icon-btn" title="Run">▶</button>
      <button className="menubar-icon-btn" title="Stop">■</button>
      <button className="menubar-icon-btn" title="Settings">⚙</button>
    </header>
  );
}
