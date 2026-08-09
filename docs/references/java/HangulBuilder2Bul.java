package com.seojin.hangul;

import java.util.HashMap;
import java.util.Stack;

public class HangulBuilder2Bul extends HangulBuilder {

	public HangulBuilder2Bul() {
		super();
		
		combinedJamo.put(Jung.O, new HashMap<Object, Object>());
		combinedJamo.get(Jung.O).put(Jung.A, Jung.WA);
		combinedJamo.get(Jung.O).put(Jung.AE, Jung.WAE);
		combinedJamo.get(Jung.O).put(Jung.I, Jung.OE);

		combinedJamo.put(Jung.U, new HashMap<Object, Object>());
		combinedJamo.get(Jung.U).put(Jung.EO, Jung.WEO);
		combinedJamo.get(Jung.U).put(Jung.E, Jung.WE);
		combinedJamo.get(Jung.U).put(Jung.I, Jung.WI);
		
		combinedJamo.put(Jung.EU, new HashMap<Object, Object>());
		combinedJamo.get(Jung.EU).put(Jung.I, Jung.YI);
		
		combinedJamo.put(Jong.G, new HashMap<Object, Object>());
		combinedJamo.get(Jong.G).put(Jong.S, Jong.GS);
		
		combinedJamo.put(Jong.N, new HashMap<Object, Object>());
		combinedJamo.get(Jong.N).put(Jong.J, Jong.NJ);
		combinedJamo.get(Jong.N).put(Jong.H, Jong.NH);
		
		combinedJamo.put(Jong.L, new HashMap<Object, Object>());
		combinedJamo.get(Jong.L).put(Jong.G, Jong.LG);
		combinedJamo.get(Jong.L).put(Jong.M, Jong.LM);
		combinedJamo.get(Jong.L).put(Jong.B, Jong.LB);
		combinedJamo.get(Jong.L).put(Jong.S, Jong.LS);
		combinedJamo.get(Jong.L).put(Jong.T, Jong.LT);
		combinedJamo.get(Jong.L).put(Jong.P, Jong.LP);
		combinedJamo.get(Jong.L).put(Jong.H, Jong.LH);
		
		combinedJamo.put(Jong.B, new HashMap<Object, Object>());
		combinedJamo.get(Jong.B).put(Jong.S, Jong.BS);
	}

	@SuppressWarnings("unchecked")
	@Override
	public HangulChar addJamo(Object jamo) {
		if (isJamo(jamo)) { // 객체자 자모면
			mJamoStack.push(jamo); // 일단 자모 넣고 조합 시도
			if (buildHangul()) { // 조합에 성공했으면
				return null;
			} else { // 조합에 실패 했으면
				mJamoStack.pop(); // 넣었던 자모 빼고 
				// 마지막 자모가 종성이고 넣을려던 자모가 중성이면 (도깨비불 현상)
				// 사곽ㅏ -> 사과가
				if (isJong(mJamoStack.peek()) && isJung(jamo)) {
					Jong j = (Jong)mJamoStack.pop(); // 마지막 종성 빼고 
					buildHangul(); // 다시 조합
					// 넘겨줄 한글 객체
					HangulChar completeHangul = new HangulChar(getCho(),getJung(),getJong());
					mLastJamoStack = (Stack<Object>) mJamoStack.clone(); // 스택 복사
					mJamoStack.clear(); // 스택 비움
					mJamoStack.push(j.toCho()); // 뺐던 종성을 초성으로 넣고
					mJamoStack.push(jamo); // 애초에 넣으려던 중성도 넣음
					
					clear(); // 자모 초기화
					buildHangul(); // 다시 조합
					
					return completeHangul; // 한글 객체 넘김
				} else { // 단순 조합 실패면 
					buildHangul();  // 다시 조합
					// 넘겨줄 한글 객체
					HangulChar completeHangul = new HangulChar(getCho(),getJung(),getJong());
					mLastJamoStack = (Stack<Object>) mJamoStack.clone(); // 스택 복사
					mJamoStack.clear(); // 스택 비움
					mJamoStack.push(jamo); // 애초에 넣으려던 자모를 넣음
					
					clear(); // 자모 초기화
					buildHangul(); // 다시 조합
					
					return completeHangul; // 한글 객체 넘김
				}
			}
		} else {
			return null;
		}
	}

	@Override
	public boolean buildHangul () {
		Object lastPrevJamo = null, lastJamo;

		if (mJamoStack.empty()) { // 스택이 비어있으면
			clear();
			return true;
		} else
			lastJamo = mJamoStack.peek();

		if (mJamoStack.size() > 1) { // 스택이 1개 이상이면
			lastPrevJamo = mJamoStack.get(mJamoStack.size()-2);
		}
		
		// 중성 다음으로 초성이 들어오면 종성으로 
		if (isFilledJung() && isCho(lastJamo)) {
			try {
				// 초성으로 들어온 자모 빼서 종성으로 넣고
				mJamoStack.pop();
				mJamoStack.push(((Cho)lastJamo).toJong());
				lastJamo = mJamoStack.peek();
			} catch (IllegalArgumentException e) { // 종성으로 변환 실패하면
				mJamoStack.push(lastJamo);
				return false;
			}
		}
		
		// 초성이 없고 중성 다음에 종성이 오면 
		if (!isFilledCho() && isJung(lastPrevJamo) && isJong(lastJamo))
			return false;
		// 종성 다음에 중성이 오면
		if (isJong(lastPrevJamo) && isJung(lastJamo))
			return false;
		
		if (!buildCho())
			return false;
		if (!buildJung())
			return false;
		if (!buildJong())
			return false;
		
		return true;
	}
}
