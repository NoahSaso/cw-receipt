# cw-receipt

This contract accepts payments associated with arbitrary IDs and passes them
through to an output address. It accumulates the payments according to their ID
into receipts and totals. This serves to record payments like a receipt,
allowing one to track when a payment was made and how much it was for.
