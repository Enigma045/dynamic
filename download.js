// Change this to your hosted server URL
const SERVER_URL = "https://your-server-domain.com";

async function fetchFiles() {
    try {
        const response = await fetch(`${SERVER_URL}/files`);
        const files = await response.json();

        const list = document.getElementById('fileList');
        list.innerHTML = "";

        files.forEach(filename => {
            const li = document.createElement('li');
            const a = document.createElement('a');
            a.href = `${SERVER_URL}/download/${filename}`;
            a.textContent = filename;
            a.download = filename;
            li.appendChild(a);
            list.appendChild(li);
        });
    } catch (err) {
        console.error(err);
    }
}

window.onload = fetchFiles;
