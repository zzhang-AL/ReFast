import { HotkeySettings } from "./components/HotkeySettings";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./styles.css";

function HotkeySettingsApp() {
  const handleClose = async () => {
    try {
      const window = getCurrentWindow();
      await window.close();
    } catch (error) {
      console.error("Failed to close window:", error);
    }
  };

  return (
    <div 
      className="h-screen w-screen" 
      style={{ 
        backgroundColor: '#f3f4f6', 
        margin: 0, 
        padding: 0,
        overflow: 'hidden'
      }}
    >
      <HotkeySettings onClose={handleClose} />
    </div>
  );
}

export default HotkeySettingsApp;

