// Test script to check splitter functionality
// Copy and paste this into the browser console at http://localhost:9440

console.log('Testing splitter functionality...');

// Find the splitter element
const splitter = document.querySelector('[title="Drag to resize panels"]');
if (!splitter) {
  console.error('Splitter element not found');
} else {
  console.log('Splitter element found:', splitter);

  // Test mouse events
  console.log('Simulating mousedown event...');
  const mouseDownEvent = new MouseEvent('mousedown', {
    clientX: 100,
    clientY: 400,
    bubbles: true,
    cancelable: true
  });

  splitter.dispatchEvent(mouseDownEvent);

  setTimeout(() => {
    console.log('Simulating mousemove event...');
    const mouseMoveEvent = new MouseEvent('mousemove', {
      clientX: 100,
      clientY: 450,
      bubbles: true,
      cancelable: true
    });

    document.dispatchEvent(mouseMoveEvent);

    setTimeout(() => {
      console.log('Simulating mouseup event...');
      const mouseUpEvent = new MouseEvent('mouseup', {
        clientX: 100,
        clientY: 450,
        bubbles: true,
        cancelable: true
      });

      document.dispatchEvent(mouseUpEvent);
    }, 100);
  }, 100);
}

// Also check the dashboard container
const container = document.querySelector('[data-testid="dashboard-container"]');
if (container) {
  console.log('Dashboard container found:', container);
  console.log('Container height:', container.getBoundingClientRect().height);
} else {
  console.error('Dashboard container not found');
}