import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
	const [sendOscMsg, setSendOscMsg] = useState("");

	async function sendOsc() {
		try {
			await invoke("send_osc", {
				address: "/test/message",
				value: "Hello OSC",
				targetIp: "127.0.0.1",
				targetPort: 9000
			});
			setSendOscMsg("OSC message sent successfully!");
		} catch (error) {
			setSendOscMsg(`Error: ${error}`);
		}
	}

	return (
		<main className="container">
			<h1>OSC TEST</h1>
			
			<div>
				<button onClick={sendOsc}>Send Custom OSC</button>
				<p>{sendOscMsg}</p>
			</div>
		</main>
	);
}

export default App;
