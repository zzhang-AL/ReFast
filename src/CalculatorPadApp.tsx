import { CalculatorPadWindow } from "./components/CalculatorPadWindow";
import "./styles.css";

function CalculatorPadApp() {
  return (
    <div
      className="h-screen w-screen"
      style={{
        backgroundColor: "#f9fafb",
        margin: 0,
        padding: 0,
        height: "100vh",
        width: "100vw",
      }}
    >
      <CalculatorPadWindow />
    </div>
  );
}

export default CalculatorPadApp;

