use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::thread;
use std::env;

const UPLOAD_DIR: &str = "uploads";

// Embed the HTML/JS files directly
const UPLOAD_HTML: &str = r#"<!DOCTYPE html>
<html>
<head><title>Upload File</title></head>
<body>
<h2>Upload a file</h2>
<input type="file" id="fileInput" />
<button id="submit">Upload</button>
<script>
const SERVER_URL = '';
const fileInput = document.getElementById('fileInput');
const submitButton = document.getElementById('submit');
submitButton.addEventListener('click', async (e) => {
    e.preventDefault();
    const file = fileInput.files[0];
    if (!file) { alert("Select a file first!"); return; }
    const formData = new FormData();
    formData.append('file', file);
    try {
        const res = await fetch(`${SERVER_URL}/upload_file`, { method: 'POST', body: formData });
        const text = await res.text();
        alert(text);
    } catch (err) { console.error(err); alert("Upload failed!"); }
});
</script>
</body>
</html>"#;

const DOWNLOAD_HTML: &str = r#"<!DOCTYPE html>
<html>
<head><title>Download Files</title></head>
<body>
<h2>Uploaded Files</h2>
<ul id="fileList"></ul>
<script>
const SERVER_URL = '';
async function fetchFiles() {
    try {
        const res = await fetch(`${SERVER_URL}/files`);
        const files = await res.json();
        const list = document.getElementById('fileList');
        list.innerHTML = "";
        files.forEach(f => {
            const li = document.createElement('li');
            const a = document.createElement('a');
            a.href = `${SERVER_URL}/download/${f}`;
            a.textContent = f;
            a.download = f;
            li.appendChild(a);
            list.appendChild(li);
        });
    } catch(e){ console.error(e); }
}
window.onload = fetchFiles;
</script>
</body>
</html>"#;

fn handle_client(mut stream: TcpStream) {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 1024];

    // Read request headers
    loop {
        let n = match stream.read(&mut temp) { Ok(0)=> return, Ok(n)=>n, Err(_) => return };
        buffer.extend_from_slice(&temp[..n]);
        if buffer.windows(4).any(|w| w==b"\r\n\r\n") { break; }
    }

    let request_text = String::from_utf8_lossy(&buffer);
    let request_line = request_text.lines().next().unwrap_or("");

    // Serve upload.html
    if request_line.starts_with("GET /upload.html") || request_line.starts_with("GET /") {
        let response = format!(
            "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
            UPLOAD_HTML.len(),
            UPLOAD_HTML
        );
        stream.write_all(response.as_bytes()).unwrap();
        return;
    }

    // Serve download.html
    if request_line.starts_with("GET /download.html") {
        let response = format!(
            "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
            DOWNLOAD_HTML.len(),
            DOWNLOAD_HTML
        );
        stream.write_all(response.as_bytes()).unwrap();
        return;
    }

    // Serve uploaded files
    if request_line.starts_with("GET /download/") {
        let filename = request_line.split_whitespace().nth(1).unwrap().trim_start_matches("/download/");
        let filepath = format!("{}/{}", UPLOAD_DIR, filename);
        if Path::new(&filepath).exists() {
            if let Ok(file_data) = fs::read(&filepath) {
                let response = format!(
                    "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=\"{}\"\r\nContent-Length: {}\r\n\r\n",
                    filename, file_data.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
                stream.write_all(&file_data).unwrap();
            }
        } else {
            stream.write_all(b"HTTP/1.1 404 NOT FOUND\r\n\r\nFile not found").unwrap();
        }
        return;
    }

    // List uploaded files
    if request_line.starts_with("GET /files") {
        let mut files_list = Vec::new();
        if let Ok(entries) = fs::read_dir(UPLOAD_DIR) {
            for entry in entries.flatten() {
                let filename = entry.file_name().into_string().unwrap_or_default();
                files_list.push(filename);
            }
        }
        let json = serde_json::to_string(&files_list).unwrap_or("[]".to_string());
        let response = format!(
            "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            json.len(),
            json
        );
        stream.write_all(response.as_bytes()).unwrap();
        return;
    }

    // Handle file upload
    if request_line.starts_with("POST /upload_file") {
        let header_end = buffer.windows(4).position(|w| w==b"\r\n\r\n").unwrap();
        let headers = String::from_utf8_lossy(&buffer[..header_end]);
        let mut body = buffer[header_end+4..].to_vec();

        // Get boundary
        let boundary = headers.lines()
            .find(|l| l.contains("Content-Type: multipart/form-data"))
            .and_then(|l| l.split("boundary=").nth(1))
            .unwrap()
            .trim();
        let boundary_bytes = format!("--{}", boundary).as_bytes().to_vec();

        while !body.ends_with(&boundary_bytes) {
            let n = match stream.read(&mut temp) { Ok(0)=>break, Ok(n)=>n, Err(_) => break };
            body.extend_from_slice(&temp[..n]);
        }

        let part_start = body.windows(4).position(|w| w==b"\r\n\r\n").map(|p|p+4).unwrap_or(0);
        let part_headers = String::from_utf8_lossy(&body[..part_start-4]);
        let filename = part_headers.lines().find(|l| l.contains("filename="))
            .and_then(|l| l.split("filename=\"").nth(1))
            .and_then(|s| s.split('"').next())
            .unwrap_or("upload.bin");

        let file_end = body.windows(boundary_bytes.len()).rposition(|w| w==boundary_bytes).map(|pos| pos-2).unwrap_or(body.len());
        fs::create_dir_all(UPLOAD_DIR).unwrap();
        let filepath = format!("{}/{}", UPLOAD_DIR, filename);
        if file_end > part_start { fs::write(&filepath, &body[part_start..file_end]).unwrap(); }

        let response = "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\n\r\nFile uploaded successfully";
        stream.write_all(response.as_bytes()).unwrap();
        return;
    }

    // 404 fallback
    stream.write_all(b"HTTP/1.1 404 NOT FOUND\r\n\r\nPage not found").unwrap();
}

fn main() {
    let port = env::var("PORT").unwrap_or("8080".to_string());
    let addr = format!("0.0.0.0:{}", port);

    let listener = TcpListener::bind(&addr).unwrap();
    println!("Server running on {}", addr);

    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            std::thread::spawn(|| handle_client(stream));
        }
    }
}
