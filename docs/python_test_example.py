import unittest
import requests
import json
from imap_api import app
import time
import logging
import timeit
from datetime import datetime
from unittest.runner import TextTestRunner
from unittest.result import TestResult
import statistics

logger = logging.getLogger(__name__)

def verify_email_moved(uid, source_folder, dest_folder, message_id):
    """Verify that an email has been successfully moved using Message-ID"""
    try:
        # Check source folder
        response = requests.get(f"{BASE_URL}/emails/{source_folder}")
        if response.status_code == 200:
            emails = response.json()['emails']
            for email in emails:
                if email['message_id'] == message_id:
                    logger.error(f"Email still exists in source folder {source_folder}")
                    return False
        
        # Check destination folder
        response = requests.get(f"{BASE_URL}/emails/{dest_folder}")
        if response.status_code == 200:
            emails = response.json()['emails']
            for email in emails:
                if email['message_id'] == message_id:
                    logger.info(f"Email verified in destination folder {dest_folder}")
                    return True
        
        logger.error(f"Email not found in either source or destination folder")
        return False
    except Exception as e:
        logger.error(f"Error during verification: {str(e)}")
        return False

class BenchmarkTestResult(TestResult):
    """Custom test result class that collects timing information"""
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.test_timings = {}
        self.test_start_time = None
        self.api_timings = {}
        
    def startTest(self, test):
        self.test_start_time = timeit.default_timer()
        self.api_timings[test.id()] = []
        super().startTest(test)
        
    def stopTest(self, test):
        elapsed = timeit.default_timer() - self.test_start_time
        self.test_timings[test.id()] = elapsed
        super().stopTest(test)
        
    def record_api_call(self, test_id, endpoint, method, elapsed):
        """Record timing for an individual API call"""
        self.api_timings[test_id].append({
            'endpoint': endpoint,
            'method': method,
            'elapsed': elapsed
        })

class BenchmarkTestRunner(TextTestRunner):
    """Custom test runner that uses BenchmarkTestResult"""
    def __init__(self, *args, **kwargs):
        kwargs['resultclass'] = BenchmarkTestResult
        super().__init__(*args, **kwargs)
        
    def run(self, test):
        result = super().run(test)
        self._print_benchmark_report(result)
        return result
    
    def _print_benchmark_report(self, result):
        """Print detailed benchmark report"""
        print("\n=== BENCHMARK REPORT ===\n")
        
        # Overall statistics
        total_time = sum(result.test_timings.values())
        test_count = len(result.test_timings)
        print(f"Total Tests: {test_count}")
        print(f"Total Time: {total_time:.3f}s")
        print(f"Average Time per Test: {total_time/test_count:.3f}s")
        print("\nTest Timings (sorted by duration):")
        print("-" * 80)
        
        # Sort tests by duration
        sorted_tests = sorted(
            result.test_timings.items(),
            key=lambda x: x[1],
            reverse=True
        )
        
        # Print individual test timings
        for test_id, duration in sorted_tests:
            print(f"{test_id:<60} {duration:>8.3f}s")
            
            # Print API call details for this test
            api_calls = result.api_timings.get(test_id, [])
            if api_calls:
                print("\n  API Calls:")
                for call in api_calls:
                    print(f"    {call['method']:<6} {call['endpoint']:<40} {call['elapsed']:>8.3f}s")
                
                # Calculate API call statistics
                call_times = [call['elapsed'] for call in api_calls]
                if call_times:
                    print(f"    Total API calls: {len(call_times)}")
                    print(f"    Average API call time: {statistics.mean(call_times):.3f}s")
                    print(f"    Max API call time: {max(call_times):.3f}s")
                    print(f"    Min API call time: {min(call_times):.3f}s")
                    if len(call_times) > 1:
                        print(f"    Std Dev API call time: {statistics.stdev(call_times):.3f}s")
            print("-" * 80)
        
        # Print summary statistics
        print("\nSummary Statistics:")
        print(f"Fastest Test: {min(result.test_timings.values()):.3f}s")
        print(f"Slowest Test: {max(result.test_timings.values()):.3f}s")
        print(f"Mean Test Time: {statistics.mean(result.test_timings.values()):.3f}s")
        if len(result.test_timings) > 1:
            print(f"Std Dev Test Time: {statistics.stdev(result.test_timings.values()):.3f}s")

class APICallWrapper:
    """Wrapper to measure API call timings"""
    def __init__(self, client, test_case):
        self.client = client
        self.test_case = test_case
    
    def _record_timing(self, method, endpoint, start_time):
        elapsed = timeit.default_timer() - start_time
        if hasattr(self.test_case, '_outcome') and hasattr(self.test_case._outcome, 'result'):
            result = self.test_case._outcome.result
            if isinstance(result, BenchmarkTestResult):
                result.record_api_call(
                    self.test_case.id(),
                    endpoint,
                    method,
                    elapsed
                )
        return elapsed
        
    def get(self, endpoint, *args, **kwargs):
        start = timeit.default_timer()
        response = self.client.get(endpoint, *args, **kwargs)
        self._record_timing('GET', endpoint, start)
        return response
        
    def post(self, endpoint, *args, **kwargs):
        start = timeit.default_timer()
        response = self.client.post(endpoint, *args, **kwargs)
        self._record_timing('POST', endpoint, start)
        return response
        
    def delete(self, endpoint, *args, **kwargs):
        start = timeit.default_timer()
        response = self.client.delete(endpoint, *args, **kwargs)
        self._record_timing('DELETE', endpoint, start)
        return response

class TestIMAPAPI(unittest.TestCase):
    def setUp(self):
        self.base_url = "http://localhost:5000"
        self.client = APICallWrapper(app.test_client(), self)
        
    def test_list_folders(self):
        """Test listing all IMAP folders"""
        response = self.client.get('/folders')
        self.assertEqual(response.status_code, 200)
        data = json.loads(response.data)
        self.assertIsInstance(data, dict)
        self.assertIn('folders', data)
        self.assertIsInstance(data['folders'], list)
        self.assertIn('INBOX', data['folders'])
        
    def test_list_emails(self):
        """Test listing emails in INBOX"""
        response = self.client.get('/emails/INBOX')
        self.assertEqual(response.status_code, 200)
        data = json.loads(response.data)
        self.assertIsInstance(data, dict)
        self.assertIn('emails', data)
        self.assertIsInstance(data['emails'], list)
        if data['emails']:
            email = data['emails'][0]
            self.assertIn('uid', email)
            self.assertIn('subject', email)
            self.assertIn('from', email)
            self.assertIn('date', email)
        
    def test_list_unread_emails(self):
        """Test listing unread emails in INBOX"""
        response = self.client.get('/emails/INBOX/unread')
        self.assertEqual(response.status_code, 200)
        data = json.loads(response.data)
        self.assertIsInstance(data, dict)
        self.assertIn('emails', data)
        self.assertIsInstance(data['emails'], list)
        if data['emails']:
            email = data['emails'][0]
            self.assertIn('uid', email)
            self.assertIn('subject', email)
            self.assertIn('from', email)
            self.assertIn('date', email)
        
    def test_get_single_email(self):
        """Test getting a single email by UID"""
        # First get list of emails to get a valid UID
        response = self.client.get('/emails/INBOX')
        data = json.loads(response.data)
        if not data['emails']:  # Skip test if no emails
            self.skipTest("No emails in INBOX to test with")
            
        # Get the actual UID from the first email
        first_email = data['emails'][0]
        uid = first_email['uid']
        
        # Now try to get this specific email
        response = self.client.get(f'/emails/INBOX/{uid}')
        self.assertEqual(response.status_code, 200)
        email_data = json.loads(response.data)
        self.assertIn('subject', email_data)
        self.assertIn('from', email_data)
        self.assertIn('body', email_data)
        self.assertIn('date', email_data)
            
    def test_move_email(self):
        """Test moving an email between testing folders"""
        # First check both testing folders
        response_a = self.client.get('/emails/INBOX.TestingBoxA')
        data_a = json.loads(response_a.data)
        response_b = self.client.get('/emails/INBOX.TestingBoxB')
        data_b = json.loads(response_b.data)
        
        # Determine which folder has emails to move
        source_folder = None
        dest_folder = None
        emails = None
        
        if data_a.get('emails') and len(data_a['emails']) > 0:
            source_folder = 'INBOX.TestingBoxA'
            dest_folder = 'INBOX.TestingBoxB'
            emails = data_a['emails']
        elif data_b.get('emails') and len(data_b['emails']) > 0:
            source_folder = 'INBOX.TestingBoxB'
            dest_folder = 'INBOX.TestingBoxA'
            emails = data_b['emails']
        else:
            # If both folders are empty, try to use INBOX to INBOX.Misc for backwards compatibility
            response = self.client.get('/emails/INBOX')
            data = json.loads(response.data)
            if data.get('emails') and len(data['emails']) > 0:
                source_folder = 'INBOX'
                dest_folder = 'INBOX.Misc'
                emails = data['emails']
            else:
                self.skipTest("No emails found in any testing folders")
        
        # Get the first email from the source folder
        first_email = emails[0]
        uid = first_email['uid']
        message_id = first_email['message_id']
        
        # Now try to move this specific email
        move_data = {
            "uid": uid,
            "source_folder": source_folder,
            "dest_folder": dest_folder
        }
        response = self.client.post('/emails/move', 
                                  json=move_data,
                                  content_type='application/json')
        self.assertEqual(response.status_code, 200)
        result = json.loads(response.data)
        self.assertIn('message', result)
        self.assertIn('Moved email', result['message'])
        
        # Wait for the IMAP server to process the move
        time.sleep(2)  # Wait 2 seconds for the move to complete
        
        # Verify the email is in the destination folder by checking Message-ID
        response = self.client.get(f'/emails/{dest_folder}')
        self.assertEqual(response.status_code, 200)
        data = json.loads(response.data)
        self.assertIsInstance(data, dict)
        self.assertIn('emails', data)
        
        # Check if any email in destination folder matches the Message-ID
        email_found = False
        for email in data['emails']:
            if email['message_id'] == message_id:
                email_found = True
                break
        self.assertTrue(email_found, f"Email not found in {dest_folder} after move")
            
    def test_invalid_folder(self):
        """Test accessing an invalid folder"""
        response = self.client.get('/emails/NONEXISTENT')
        self.assertEqual(response.status_code, 404)
        
    def test_invalid_uid(self):
        """Test accessing an invalid email UID"""
        response = self.client.get('/emails/INBOX/999999999')
        self.assertEqual(response.status_code, 404)
        data = json.loads(response.data)
        self.assertIn('error', data)
        
    def test_invalid_move(self):
        """Test invalid move operations"""
        # Test missing required fields
        move_data = {"uid": "123"}  # Missing source and dest folders
        response = self.client.post('/emails/move', 
                                  json=move_data,
                                  content_type='application/json')
        self.assertEqual(response.status_code, 400)
        
        # Test invalid destination folder
        move_data = {
            "uid": "123",
            "source_folder": "INBOX",
            "dest_folder": "NONEXISTENT"
        }
        response = self.client.post('/emails/move', 
                                  json=move_data,
                                  content_type='application/json')
        self.assertEqual(response.status_code, 404)
        
    def test_homepage(self):
        """Test the homepage returns successfully"""
        response = self.client.get('/')
        self.assertEqual(response.status_code, 200)
        self.assertIn(b'IMAP API', response.data)
        
    def test_api_docs(self):
        """Test the API documentation endpoint"""
        response = self.client.get('/api-docs')
        self.assertEqual(response.status_code, 200)
        data = json.loads(response.data)
        self.assertIn('message', data)
        self.assertIn('endpoints', data)
        self.assertIsInstance(data['endpoints'], list)
        # Verify each endpoint has required fields
        for endpoint in data['endpoints']:
            self.assertIn('endpoint', endpoint)
            self.assertIn('method', endpoint)
            self.assertIn('description', endpoint)

    def test_delete_non_empty_folder(self):
        """Test the workflow of safely deleting a folder that contains emails"""
        # Create a test folder
        folder_name = "INBOX.TestDeleteWorkflow"
        create_data = {"name": folder_name}
        response = self.client.post('/folders',
                                  json=create_data,
                                  content_type='application/json')
        self.assertEqual(response.status_code, 201)
        
        # Create a test email in the folder
        email_data = {
            "subject": "Test Email for Deletion Workflow",
            "body": {
                "text_plain": "This is a test email",
                "text_html": "<p>This is a test email</p>"
            },
            "to": ["test@example.com"]
        }
        response = self.client.post(f'/emails/{folder_name}',
                                  json=email_data,
                                  content_type='application/json')
        self.assertEqual(response.status_code, 201)
        data = json.loads(response.data)
        message_id = data['message_id']
        
        # Verify folder is not empty
        response = self.client.get(f'/folders/{folder_name}/stats')
        self.assertEqual(response.status_code, 200)
        data = json.loads(response.data)
        self.assertEqual(data['total_messages'], 1)
        
        # Try to delete non-empty folder (should fail)
        response = self.client.delete(f'/folders/{folder_name}')
        self.assertEqual(response.status_code, 400)
        data = json.loads(response.data)
        self.assertIn('error', data)
        self.assertEqual(data['error'], 'Folder not empty')
        
        # Move email to another folder (INBOX)
        move_data = {
            "uid": "1",  # First email in the folder
            "source_folder": folder_name,
            "dest_folder": "INBOX"
        }
        response = self.client.post('/emails/move',
                                  json=move_data,
                                  content_type='application/json')
        self.assertEqual(response.status_code, 200)
        
        # Wait for IMAP server to process the move
        time.sleep(2)
        
        # Verify email was moved by checking Message-ID
        response = self.client.get('/emails/INBOX')
        self.assertEqual(response.status_code, 200)
        data = json.loads(response.data)
        email_found = False
        for email in data['emails']:
            if email['message_id'] == message_id:
                email_found = True
                break
        self.assertTrue(email_found, "Email not found in INBOX after move")
        
        # Verify folder is now empty
        response = self.client.get(f'/folders/{folder_name}/stats')
        self.assertEqual(response.status_code, 200)
        data = json.loads(response.data)
        self.assertEqual(data['total_messages'], 0)
        
        # Now try to delete the empty folder (should succeed)
        response = self.client.delete(f'/folders/{folder_name}')
        self.assertEqual(response.status_code, 200)
        data = json.loads(response.data)
        self.assertIn('message', data)
        self.assertIn('deleted successfully', data['message'])
        
        # Verify folder no longer exists
        response = self.client.get('/folders')
        self.assertEqual(response.status_code, 200)
        data = json.loads(response.data)
        self.assertNotIn(folder_name, data['folders'])

if __name__ == '__main__':
    runner = BenchmarkTestRunner(verbosity=2)
    unittest.main(testRunner=runner) 