#!/usr/bin/env node

const readline = require('readline');
const http = require('http');

const API_KEY = process.env.RUSTYMAIL_API_KEY || 'test-rustymail-key-2024';
const API_URL = process.env.RUSTYMAIL_API_URL || 'http://localhost:9437';

const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout,
  terminal: false
});

// Process each line as a JSON-RPC request
rl.on('line', async (line) => {
  try {
    const request = JSON.parse(line);

    // Forward the request to the HTTP endpoint
    const response = await makeHttpRequest(request);

    // Output the response
    console.log(JSON.stringify(response));
  } catch (error) {
    console.error(JSON.stringify({
      jsonrpc: '2.0',
      error: {
        code: -32700,
        message: 'Parse error: ' + error.message
      }
    }));
  }
});

function makeHttpRequest(request) {
  return new Promise((resolve, reject) => {
    const postData = JSON.stringify({
      tool: request.method,
      parameters: request.params || {}
    });

    const options = {
      hostname: new URL(API_URL).hostname,
      port: new URL(API_URL).port || 80,
      path: '/api/dashboard/mcp/execute',
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Content-Length': Buffer.byteLength(postData),
        'X-API-Key': API_KEY
      }
    };

    const req = http.request(options, (res) => {
      let data = '';

      res.on('data', (chunk) => {
        data += chunk;
      });

      res.on('end', () => {
        try {
          const responseData = JSON.parse(data);

          // Transform the response to JSON-RPC format
          if (responseData.data) {
            resolve({
              jsonrpc: '2.0',
              id: request.id,
              result: responseData.data
            });
          } else if (responseData.error) {
            resolve({
              jsonrpc: '2.0',
              id: request.id,
              error: {
                code: -32603,
                message: responseData.error
              }
            });
          } else {
            resolve({
              jsonrpc: '2.0',
              id: request.id,
              result: responseData
            });
          }
        } catch (e) {
          resolve({
            jsonrpc: '2.0',
            id: request.id,
            error: {
              code: -32603,
              message: 'Invalid response: ' + e.message
            }
          });
        }
      });
    });

    req.on('error', (e) => {
      resolve({
        jsonrpc: '2.0',
        id: request.id,
        error: {
          code: -32603,
          message: 'Network error: ' + e.message
        }
      });
    });

    req.write(postData);
    req.end();
  });
}

// Handle process termination gracefully
process.on('SIGINT', () => {
  process.exit(0);
});

process.on('SIGTERM', () => {
  process.exit(0);
});