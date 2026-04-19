/**
 * 한글 조합 취상위 클래스
 * @author "KiHyeon Seo" <from104@gmail.com>
 * @version 0.0.1
 */
package com.seojin.hangul;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.Stack;

public abstract class HangulBuilder extends HangulChar {
	// 자모가 입력되는 순서대로 저장하는 스택
	protected Stack<Object> mJamoStack;
	// 직전 스택
	protected Stack<Object> mLastJamoStack;
	
	// 자모 조합 테이블 ({O={A=WA,AE=WAE....}, 3개 이상 자모는 2개씩 나누어서)
	protected HashMap<Object,HashMap<Object,Object>> combinedJamo;
	
	/**
	 * 기본 생성자
	 */
	HangulBuilder () {
		mJamoStack = new Stack<Object>();
		mLastJamoStack = new Stack<Object>();
		
		combinedJamo = new HashMap<Object,HashMap<Object,Object>>();
	}
	
	/**
	 * 한글 자모 입력. 
	 * @param 자모 객체
	 * @return  조합이 완료되면 완성된 한글 객체를 넘겨주고 조합중이면 null을 넘겨줌.
	 */
	public abstract HangulChar addJamo(Object jamo) ;

	/**
	 * 한글 자모 삭제. 마지막으로 입력했던 자모부터 삭제 후에 다시 조합.
	 * @return 삭제된 자모 객체 또는 null
	 */
	public Object removeJamo() {
		if (mJamoStack.empty()) // 스택이 비었으면
			return null;
		else { // 스택이 차있으면
			Object o = mJamoStack.pop(); // 마지막 자모 빼고
			buildHangul(); // 다시 조합
			return o;			
		}
	}

	/**
	 * 한글 초성 조합
	 * @return 성공 또는 실패
	 */
	protected boolean buildCho () {		
		ArrayList<Object> jamoArray = new ArrayList<Object>();
		Object firstJamo, secondJamo;

		// 초성만 걸러냄
		for (Object o : mJamoStack.toArray()) {
			if (isCho(o))
				jamoArray.add(o);
		}

		if (jamoArray.isEmpty()) // 초성이 없으면 
			clearCho();
		else { // 초성이 있으면
			setCho((Cho)jamoArray.get(0)); // 첫번째 초성 정하기
			if (jamoArray.size() > 1) { // 초성이 1개보다 많으면
				jamoArray.remove(0); // 첫번째 초성 지우기
				for (Object o : jamoArray.toArray()) { // 두번째 부터 반복
					firstJamo = getCho(); // 마지막으로 조합된 초성과
					secondJamo = o; // 조합될 두번째 초성
					// 조합되는 자모가 있으면
					if (combinedJamo.containsKey(firstJamo) &&
						combinedJamo.get(firstJamo).containsKey(secondJamo)) {
						setCho((Cho)combinedJamo.get(firstJamo).get(secondJamo));
					} else { // 없으면 실패
						return false;
					}					
				}
			}
		} 

		
		return true;		
	}
	
	/**
	 * 한글 중성 조합
	 * @return 성공 또는 실패
	 */
	protected boolean buildJung () {		
		ArrayList<Object> jamoArray = new ArrayList<Object>();
		Object firstJamo, secondJamo;

		// 중성 분리 
		for (Object o : mJamoStack.toArray()) {
			if (isJung(o))
				jamoArray.add(o);
		}

		if (jamoArray.isEmpty()) 
			clearJung();
		else {
			setJung((Jung)jamoArray.get(0));
			if (jamoArray.size() > 1) {
				jamoArray.remove(0);
				for (Object o : jamoArray.toArray()) {
					firstJamo = getJung();
					secondJamo = o;
					if (combinedJamo.containsKey(firstJamo) &&
						combinedJamo.get(firstJamo).containsKey(secondJamo)) {
						setJung((Jung)combinedJamo.get(firstJamo).get(secondJamo));
					} else {
						return false;
					}					
				}
			}
		} 

		return true;		
	}
	
	/**
	 * 한글 종성 조합
	 * @return 성공 또는 실패
	 */
	protected boolean buildJong () {		
		ArrayList<Object> jamoArray = new ArrayList<Object>();
		Object firstJamo, secondJamo;

		// 종성 분리
		for (Object o : mJamoStack.toArray()) {
			if (isJong(o))
				jamoArray.add(o);
		}

		if (jamoArray.isEmpty()) 
			clearJong();
		else {
			setJong((Jong)jamoArray.get(0));
			if (jamoArray.size() > 1) {
				jamoArray.remove(0);
				for (Object o : jamoArray.toArray()) {
					firstJamo = getJong();
					secondJamo = o;
					if (combinedJamo.containsKey(firstJamo) &&
						combinedJamo.get(firstJamo).containsKey(secondJamo)) {
						setJong((Jong)combinedJamo.get(firstJamo).get(secondJamo));
					} else {
						return false;
					}					
				}
			}
		} 

		return true;		
	}
	
	/**
	 * 스택에 쌓인 자모들로 한글 조합
	 * @return 성공 또는 실패
	 */
	public abstract boolean buildHangul ();
	
	/**
	 * 강제로 조합 완료
	 * @return 완성된 한글 객체 또는 null
	 */
	public HangulChar forceBuildHangul() {
		if (isBuild()) {
			buildHangul();
			HangulChar completeHangul = new HangulChar(getCho(),getJung(),getJong());
			clear();
			mJamoStack.clear();
			mLastJamoStack.clear();
			
			return completeHangul;
		} else 
			return null;
	}

	
	/**
	 * 조합 중인지 여부
	 * @return
	 */
	public boolean isBuild() {
		return !mJamoStack.empty();
	}
}
