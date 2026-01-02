// Change this to your hosted server URL
const SERVER_URL = "https://your-server-domain.com";

const fileInput = document.getElementById('fileInput');
const submitButton = document.getElementById('submit');

submitButton.addEventListener('click', async (e) => {
    e.preventDefault();
    const file = fileInput.files[0];
    if (!file) {
        alert("Select a file first!");
        return;
    }

    const formData = new FormData();
    formData.append('file', file);

    try {
        const response = await fetch(`${SERVER_URL}/upload_file`, {
            method: 'POST',
            body: formData
        });
        const text = await response.text();
        console.log(text);
        alert("Upload complete! Go to download page.");
    } catch (err) {
        console.error(err);
        alert("Upload failed!");
    }
});
