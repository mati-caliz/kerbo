class A:
    def f(self):
        for char in ('\U0002A6E0\U0002FA20\U0003134B\U0003FFFD'
                     '\uFA6E\uFADA'): # New in 5.2.0
            self.assertEqual(eaw(char), 'W')
            self.assertIs(self.db.name(char, None), None)
            pass
